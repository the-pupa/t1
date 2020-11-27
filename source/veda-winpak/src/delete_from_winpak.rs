use crate::common_winpak::*;
use futures::Future;
use tiberius::SqlConnection;
use tokio::runtime::current_thread;
use v_api::app::ResultCode;
use v_api::*;
use v_module::module::*;
use v_onto::datatype::Lang;
use v_onto::individual::*;

pub fn delete_from_winpak<'a>(module: &mut Module, systicket: &str, conn_str: &str, indv: &mut Individual) -> ResultCode {
    let (sync_res, info) = sync_data_to_winpak(module, conn_str, indv);
    if sync_res == ResultCode::ConnectError {
        return sync_res;
    }

    indv.set_uri("v-s:lastEditor", "cfg:VedaSystemAppointment");

    if sync_res == ResultCode::Ok {
        indv.set_uri("v-s:hasStatus", "v-s:StatusAccepted");
    } else {
        indv.set_uri("v-s:hasStatus", "v-s:StatusRejected");
        indv.add_string("v-s:errorMessage", info, Lang::RU);
    }
    indv.clear("v-s:errorMessage");

    info!("update from {}, status={:?}, info={}", indv.get_id(), sync_res, info);
    let res = module.api.update(systicket, IndvOp::Put, indv);
    if res.result != ResultCode::Ok {
        error!("fail update, uri={}, result_code={:?}", indv.get_id(), res.result);
    } else {
        info!("success update, uri={}", indv.get_id());
    }
    sync_res
}

fn sync_data_to_winpak<'a>(module: &mut Module, conn_str: &str, indv: &mut Individual) -> (ResultCode, &'a str) {
    let backward_target = indv.get_first_literal("v-s:backwardTarget");
    if backward_target.is_none() {
        error!("not found [v-s:backwardTarget] in {}", indv.get_id());
        return (ResultCode::NotFound, "исходные данные некорректны");
    }
    let backward_target = backward_target.unwrap();

    let indv_b = module.get_individual_h(&backward_target);
    if indv_b.is_none() {
        error!("not found {}", &backward_target);
        return (ResultCode::NotFound, "исходные данные некорректны");
    }
    let mut indv_b = indv_b.unwrap();

    let btype = indv_b.get_first_literal("rdf:type").unwrap_or_default();

    let has_change_kind_for_pass = indv_b.get_literals("mnd-s:hasChangeKindForPass");
    if btype != "mnd-s:Pass" && has_change_kind_for_pass.is_none() {
        error!("not found [mnd-s:hasChangeKindForPass] in {}", indv_b.get_id());
        return (ResultCode::NotFound, "исходные данные некорректны");
    }

    let has_change_kind_for_passes = has_change_kind_for_pass.unwrap_or_default();

    let wcard_number = indv_b.get_first_literal("mnd-s:cardNumber");
    if wcard_number.is_none() {
        error!("not found [mnd-s:cardNumber] in {}", indv_b.get_id());
        return (ResultCode::NotFound, "исходные данные некорректны");
    }
    let card_number = wcard_number.unwrap();

    let mut deleted_access_levels: Vec<i64> = Vec::new();
    let mut is_deleted_access_levels = false;

    if btype != "mnd-s:Pass" {
        for has_change_kind_for_pass in has_change_kind_for_passes {
            if has_change_kind_for_pass == "d:efbibmgvxpr46t1qksmtkkautw" {
                is_deleted_access_levels = true;
                get_access_level(&mut indv_b, "mnd-s:hasTemporaryAccessLevel", &mut deleted_access_levels);
            }
        }
    }

    let future = SqlConnection::connect(conn_str)
        .and_then(|conn| conn.transaction())
        .and_then(|trans| delete_access_levels(is_deleted_access_levels, 0, deleted_access_levels, card_number.to_string(), trans))
        .and_then(|trans| deactivate_card(is_deleted_access_levels, Some(get_now_00_00_00().timestamp()), card_number.to_string(), trans))
        .and_then(|trans| trans.commit());
    return match current_thread::block_on_all(future) {
        Ok(_) => (ResultCode::Ok, "данные обновлены"),
        Err(e) => {
            error!("fail execute query, err={:?}", e);
            (ResultCode::DatabaseModifiedError, "ошибка обновления")
        }
    };
}
