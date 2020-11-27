use parse_duration::parse;
use regex::Regex;
use v_api::app::ResultCode;
use v_api::IndvOp;
use v_authorization::common::Trace;
use v_az_lmdb::_authorize;
use v_ft_xapian::xapian_reader::XapianReader;
use v_module::module::{create_new_ticket, Module};
use v_module::ticket::Ticket;
use v_onto::datatype::Lang;
use v_onto::individual::Individual;
use v_search::common::{FTQuery, QueryResult};

pub const EMPTY_SHA256_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
pub const ALLOW_TRUSTED_GROUP: &str = "cfg:TrustedAuthenticationUserGroup";

#[derive(Default, Debug)]
pub(crate) struct UserStat {
    pub wrong_count_login: i32,
    pub last_wrong_login_date: i64,
    pub attempt_change_pass: i32,
    pub last_attempt_change_pass_date: i64,
}

#[derive(Debug)]
pub(crate) struct AuthConf {
    pub failed_auth_attempts: i32,
    pub failed_change_pass_attempts: i32,
    pub failed_auth_lock_period: i64,
    pub failed_pass_change_lock_period: i64,
    pub success_pass_change_lock_period: i64,
    pub ticket_lifetime: i64,
    pub secret_lifetime: i64,
    pub pass_lifetime: i64,
}

impl Default for AuthConf {
    fn default() -> Self {
        AuthConf {
            failed_auth_attempts: 2,
            failed_change_pass_attempts: 2,
            failed_auth_lock_period: 30 * 60,
            failed_pass_change_lock_period: 30 * 60,
            success_pass_change_lock_period: 24 * 60 * 60,
            ticket_lifetime: 10 * 60 * 60,
            secret_lifetime: 12 * 60 * 60,
            pass_lifetime: 90 * 24 * 60 * 60,
        }
    }
}

pub(crate) fn get_ticket_trusted(conf: &AuthConf, tr_ticket_id: Option<&str>, login: Option<&str>, xr: &mut XapianReader, module: &mut Module) -> Ticket {
    let login = login.unwrap_or_default();
    let tr_ticket_id = tr_ticket_id.unwrap_or_default();

    info!("get_ticket_trusted, login={} ticket={}", login, tr_ticket_id);

    if login.is_empty() || tr_ticket_id.len() < 6 {
        warn!("trusted authenticate: invalid login {} or ticket {}", login, tr_ticket_id);
        return Ticket::default();
    }

    let mut tr_ticket = module.get_ticket_from_db(&tr_ticket_id);

    if tr_ticket.result == ResultCode::Ok {
        let mut is_allow_trusted = false;

        let mut trace = Trace {
            acl: &mut String::new(),
            is_acl: false,
            group: &mut String::new(),
            is_group: true,
            info: &mut String::new(),
            is_info: false,
            str_num: 0,
        };

        match _authorize(&tr_ticket.user_uri, &tr_ticket.user_uri, 15, true, Some(&mut trace)) {
            Ok(_res) => {
                for gr in trace.group.split('\n') {
                    if gr == ALLOW_TRUSTED_GROUP {
                        is_allow_trusted = true;
                        break;
                    }
                }
            }
            Err(e) => error!("fail get authorization group of {}, err={}", &tr_ticket.user_uri, e),
        }

        if is_allow_trusted {
            let candidate_account_ids = get_candidate_users_of_login(login, module, xr);
            if candidate_account_ids.result_code == ResultCode::Ok && candidate_account_ids.count > 0 {
                for account_id in &candidate_account_ids.result {
                    if let Some(account) = module.get_individual(&account_id, &mut Individual::default()) {
                        let user_id = account.get_first_literal("v-s:owner").unwrap_or_default();
                        if user_id.is_empty() {
                            error!("user id is null, user_indv={}", account);
                            continue;
                        }

                        let user_login = account.get_first_literal("v-s:login").unwrap_or_default();
                        if user_login.is_empty() {
                            error!("user login {:?} not equal request login {}", user_login, login);
                            continue;
                        }

                        let mut ticket = Ticket::default();
                        create_new_ticket(login, &user_id, conf.ticket_lifetime, &mut ticket, &mut module.storage);
                        info!("trusted authenticate, result ticket={:?}", ticket);

                        return ticket;
                    }
                }
            }
        } else {
            error!("trusted authenticate: User {} must be a member of group {}", tr_ticket.user_uri, ALLOW_TRUSTED_GROUP);
        }
    } else {
        warn!("trusted authenticate: problem ticket {}", tr_ticket_id);
    }

    tr_ticket.result = ResultCode::AuthenticationFailed;
    error!("failed trusted authenticate, ticket={} login={}", tr_ticket_id, login);

    tr_ticket
}

pub(crate) fn get_candidate_users_of_login(login: &str, module: &mut Module, xr: &mut XapianReader) -> QueryResult {
    lazy_static! {
        static ref RE: Regex = Regex::new("[-]").unwrap();
    }

    let query = format!("'v-s:login' == '{}'", RE.replace_all(login, " +"));

    xr.query(FTQuery::new_with_user("cfg:VedaSystem", &query), &mut module.storage)
}

pub(crate) fn create_new_credential(systicket: &str, module: &mut Module, uses_credential: &mut Individual, account: &mut Individual) -> bool {
    let password = account.get_first_literal("v-s:password").unwrap_or_default();

    uses_credential.set_id(&(account.get_id().to_owned() + "-crdt"));
    uses_credential.set_uri("rdf:type", "v-s:Credential");
    uses_credential.set_string("v-s:password", &password, Lang::NONE);

    let res = module.api.update(systicket, IndvOp::Put, &uses_credential);
    if res.result != ResultCode::Ok {
        error!("fail update, uri={}, result_code={:?}", uses_credential.get_id(), res.result);
        return false;
    } else {
        info!("create v-s:Credential {}, res={:?}", uses_credential.get_id(), res);

        account.remove("v-s:password");
        account.set_uri("v-s:usesCredential", uses_credential.get_id());

        let res = module.api.update(&systicket, IndvOp::Put, account);
        if res.result != ResultCode::Ok {
            error!("fail update, uri={}, res={:?}", account.get_id(), res);
            return false;
        }
        info!("update user {}, res={:?}", account.get_id(), res);
    }
    true
}

pub(crate) fn remove_secret(uses_credential: &mut Individual, person_id: &str, module: &mut Module, systicket: &str) {
    if uses_credential.get_first_literal("v-s:secret").is_some() {
        uses_credential.remove("v-s:secret");

        let res = module.api.update(systicket, IndvOp::Remove, uses_credential);
        if res.result != ResultCode::Ok {
            error!("fail remove secret code for user, user={}", person_id);
        }
    }
}

pub(crate) fn read_duration_param(indv: &mut Individual, param: &str) -> Option<std::time::Duration> {
    if let Some(v) = indv.get_first_literal(param) {
        if let Ok(d) = parse(&v) {
            return Some(d);
        } else {
            error!("fail parse auth param {}", param);
        }
    }
    None
}

pub(crate) fn read_auth_configuration(module: &mut Module) -> AuthConf {
    let mut res = AuthConf::default();

    let mut node = Individual::default();

    if module.storage.get_individual("cfg:standart_node", &mut node) {
        if let Some(d) = read_duration_param(&mut node, "cfg:user_password_lifetime") {
            res.pass_lifetime = d.as_secs() as i64;
        }
        if let Some(d) = read_duration_param(&mut node, "cfg:user_ticket_lifetime") {
            res.ticket_lifetime = d.as_secs() as i64;
        }
        if let Some(d) = read_duration_param(&mut node, "cfg:secret_lifetime") {
            res.secret_lifetime = d.as_secs() as i64;
        }
        if let Some(d) = read_duration_param(&mut node, "cfg:failed_pass_change_lock_period") {
            res.failed_pass_change_lock_period = d.as_secs() as i64;
        }
        if let Some(d) = read_duration_param(&mut node, "cfg:success_pass_change_lock_period") {
            res.success_pass_change_lock_period = d.as_secs() as i64;
        }
        if let Some(d) = read_duration_param(&mut node, "cfg:failed_auth_lock_period") {
            res.failed_auth_lock_period = d.as_secs() as i64;
        }
        if let Some(v) = node.get_first_integer("cfg:failed_auth_attempts") {
            res.failed_auth_attempts = v as i32;
        }
        if let Some(v) = node.get_first_integer("cfg:failed_change_pass_attempts") {
            res.failed_change_pass_attempts = v as i32;
        }
    }

    info!("read configuration: {:?}", res);

    res
}
