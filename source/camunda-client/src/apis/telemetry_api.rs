/*
 * Camunda BPM REST API
 *
 * OpenApi Spec for Camunda BPM REST API.
 *
 * The version of the OpenAPI document: 7.14.0
 * 
 * Generated by: https://openapi-generator.tech
 */

use std::rc::Rc;
use std::borrow::Borrow;
#[allow(unused_imports)]
use std::option::Option;

use reqwest;

use super::{Error, configuration};

pub struct TelemetryApiClient {
    configuration: Rc<configuration::Configuration>,
}

impl TelemetryApiClient {
    pub fn new(configuration: Rc<configuration::Configuration>) -> TelemetryApiClient {
        TelemetryApiClient {
            configuration,
        }
    }
}

pub trait TelemetryApi {
    fn configure_telemetry(&self, telemetry_configuration_dto: Option<crate::models::TelemetryConfigurationDto>) -> Result<(), Error>;
    fn get_telemetry_configuration(&self, ) -> Result<crate::models::TelemetryConfigurationDto, Error>;
}

impl TelemetryApi for TelemetryApiClient {
    fn configure_telemetry(&self, telemetry_configuration_dto: Option<crate::models::TelemetryConfigurationDto>) -> Result<(), Error> {
        let configuration: &configuration::Configuration = self.configuration.borrow();
        let client = &configuration.client;

        let uri_str = format!("{}/telemetry/configuration", configuration.base_path);
        let mut req_builder = client.post(uri_str.as_str());

        if let Some(ref user_agent) = configuration.user_agent {
            req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
        }
        req_builder = req_builder.json(&telemetry_configuration_dto);

        // send request
        let req = req_builder.build()?;

        client.execute(req)?.error_for_status()?;
        Ok(())
    }

    fn get_telemetry_configuration(&self, ) -> Result<crate::models::TelemetryConfigurationDto, Error> {
        let configuration: &configuration::Configuration = self.configuration.borrow();
        let client = &configuration.client;

        let uri_str = format!("{}/telemetry/configuration", configuration.base_path);
        let mut req_builder = client.get(uri_str.as_str());

        if let Some(ref user_agent) = configuration.user_agent {
            req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
        }

        // send request
        let req = req_builder.build()?;

        Ok(client.execute(req)?.error_for_status()?.json()?)
    }

}
