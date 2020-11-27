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

pub struct EventSubscriptionApiClient {
    configuration: Rc<configuration::Configuration>,
}

impl EventSubscriptionApiClient {
    pub fn new(configuration: Rc<configuration::Configuration>) -> EventSubscriptionApiClient {
        EventSubscriptionApiClient {
            configuration,
        }
    }
}

pub trait EventSubscriptionApi {
    fn get_event_subscriptions(&self, event_subscription_id: Option<&str>, event_name: Option<&str>, event_type: Option<&str>, execution_id: Option<&str>, process_instance_id: Option<&str>, activity_id: Option<&str>, tenant_id_in: Option<&str>, without_tenant_id: Option<bool>, include_event_subscriptions_without_tenant_id: Option<bool>, sort_by: Option<&str>, sort_order: Option<&str>, first_result: Option<i32>, max_results: Option<i32>) -> Result<Vec<crate::models::EventSubscriptionDto>, Error>;
    fn get_event_subscriptions_count(&self, event_subscription_id: Option<&str>, event_name: Option<&str>, event_type: Option<&str>, execution_id: Option<&str>, process_instance_id: Option<&str>, activity_id: Option<&str>, tenant_id_in: Option<&str>, without_tenant_id: Option<bool>, include_event_subscriptions_without_tenant_id: Option<bool>) -> Result<crate::models::CountResultDto, Error>;
}

impl EventSubscriptionApi for EventSubscriptionApiClient {
    fn get_event_subscriptions(&self, event_subscription_id: Option<&str>, event_name: Option<&str>, event_type: Option<&str>, execution_id: Option<&str>, process_instance_id: Option<&str>, activity_id: Option<&str>, tenant_id_in: Option<&str>, without_tenant_id: Option<bool>, include_event_subscriptions_without_tenant_id: Option<bool>, sort_by: Option<&str>, sort_order: Option<&str>, first_result: Option<i32>, max_results: Option<i32>) -> Result<Vec<crate::models::EventSubscriptionDto>, Error> {
        let configuration: &configuration::Configuration = self.configuration.borrow();
        let client = &configuration.client;

        let uri_str = format!("{}/event-subscription", configuration.base_path);
        let mut req_builder = client.get(uri_str.as_str());

        if let Some(ref s) = event_subscription_id {
            req_builder = req_builder.query(&[("eventSubscriptionId", &s.to_string())]);
        }
        if let Some(ref s) = event_name {
            req_builder = req_builder.query(&[("eventName", &s.to_string())]);
        }
        if let Some(ref s) = event_type {
            req_builder = req_builder.query(&[("eventType", &s.to_string())]);
        }
        if let Some(ref s) = execution_id {
            req_builder = req_builder.query(&[("executionId", &s.to_string())]);
        }
        if let Some(ref s) = process_instance_id {
            req_builder = req_builder.query(&[("processInstanceId", &s.to_string())]);
        }
        if let Some(ref s) = activity_id {
            req_builder = req_builder.query(&[("activityId", &s.to_string())]);
        }
        if let Some(ref s) = tenant_id_in {
            req_builder = req_builder.query(&[("tenantIdIn", &s.to_string())]);
        }
        if let Some(ref s) = without_tenant_id {
            req_builder = req_builder.query(&[("withoutTenantId", &s.to_string())]);
        }
        if let Some(ref s) = include_event_subscriptions_without_tenant_id {
            req_builder = req_builder.query(&[("includeEventSubscriptionsWithoutTenantId", &s.to_string())]);
        }
        if let Some(ref s) = sort_by {
            req_builder = req_builder.query(&[("sortBy", &s.to_string())]);
        }
        if let Some(ref s) = sort_order {
            req_builder = req_builder.query(&[("sortOrder", &s.to_string())]);
        }
        if let Some(ref s) = first_result {
            req_builder = req_builder.query(&[("firstResult", &s.to_string())]);
        }
        if let Some(ref s) = max_results {
            req_builder = req_builder.query(&[("maxResults", &s.to_string())]);
        }
        if let Some(ref user_agent) = configuration.user_agent {
            req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
        }

        // send request
        let req = req_builder.build()?;

        Ok(client.execute(req)?.error_for_status()?.json()?)
    }

    fn get_event_subscriptions_count(&self, event_subscription_id: Option<&str>, event_name: Option<&str>, event_type: Option<&str>, execution_id: Option<&str>, process_instance_id: Option<&str>, activity_id: Option<&str>, tenant_id_in: Option<&str>, without_tenant_id: Option<bool>, include_event_subscriptions_without_tenant_id: Option<bool>) -> Result<crate::models::CountResultDto, Error> {
        let configuration: &configuration::Configuration = self.configuration.borrow();
        let client = &configuration.client;

        let uri_str = format!("{}/event-subscription/count", configuration.base_path);
        let mut req_builder = client.get(uri_str.as_str());

        if let Some(ref s) = event_subscription_id {
            req_builder = req_builder.query(&[("eventSubscriptionId", &s.to_string())]);
        }
        if let Some(ref s) = event_name {
            req_builder = req_builder.query(&[("eventName", &s.to_string())]);
        }
        if let Some(ref s) = event_type {
            req_builder = req_builder.query(&[("eventType", &s.to_string())]);
        }
        if let Some(ref s) = execution_id {
            req_builder = req_builder.query(&[("executionId", &s.to_string())]);
        }
        if let Some(ref s) = process_instance_id {
            req_builder = req_builder.query(&[("processInstanceId", &s.to_string())]);
        }
        if let Some(ref s) = activity_id {
            req_builder = req_builder.query(&[("activityId", &s.to_string())]);
        }
        if let Some(ref s) = tenant_id_in {
            req_builder = req_builder.query(&[("tenantIdIn", &s.to_string())]);
        }
        if let Some(ref s) = without_tenant_id {
            req_builder = req_builder.query(&[("withoutTenantId", &s.to_string())]);
        }
        if let Some(ref s) = include_event_subscriptions_without_tenant_id {
            req_builder = req_builder.query(&[("includeEventSubscriptionsWithoutTenantId", &s.to_string())]);
        }
        if let Some(ref user_agent) = configuration.user_agent {
            req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
        }

        // send request
        let req = req_builder.build()?;

        Ok(client.execute(req)?.error_for_status()?.json()?)
    }

}
