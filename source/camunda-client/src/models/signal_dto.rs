/*
 * Camunda BPM REST API
 *
 * OpenApi Spec for Camunda BPM REST API.
 *
 * The version of the OpenAPI document: 7.14.0
 * 
 * Generated by: https://openapi-generator.tech
 */




#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalDto {
    /// The name of the signal to deliver.  **Note**: This property is mandatory.
    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optionally specifies a single execution which is notified by the signal.  **Note**: If no execution id is defined the signal is broadcasted to all subscribed handlers. 
    #[serde(rename = "executionId", skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    /// A JSON object containing variable key-value pairs. Each key is a variable name and each value a JSON variable value object.
    #[serde(rename = "variables", skip_serializing_if = "Option::is_none")]
    pub variables: Option<::std::collections::HashMap<String, crate::models::VariableValueDto>>,
    /// Specifies a tenant to deliver the signal. The signal can only be received on executions or process definitions which belongs to the given tenant.  **Note**: Cannot be used in combination with executionId.
    #[serde(rename = "tenantId", skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// If true the signal can only be received on executions or process definitions which belongs to no tenant. Value may not be false as this is the default behavior.  **Note**: Cannot be used in combination with `executionId`.
    #[serde(rename = "withoutTenantId", skip_serializing_if = "Option::is_none")]
    pub without_tenant_id: Option<bool>,
}

impl SignalDto {
    pub fn new() -> SignalDto {
        SignalDto {
            name: None,
            execution_id: None,
            variables: None,
            tenant_id: None,
            without_tenant_id: None,
        }
    }
}


