use serde::{Deserialize, Deserializer, Serialize};

use super::client::CongressClient;
use super::error::ApiError;
use super::types::{BillId, BillType, Congress};

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillListResponse {
    #[serde(default)]
    pub bills: Vec<BillListItem>,
    #[serde(default)]
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    #[serde(default)]
    pub count: u32,
    pub next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillListItem {
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub congress: u32,
    #[serde(rename = "type")]
    pub r#type: String,
    pub origin_chamber: Option<String>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub number: u32,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub title: String,
    pub latest_action: Option<LatestAction>,
    pub update_date: Option<String>,
}

/// Deserialize a u32 that may come as a JSON number or a JSON string like "144".
fn deserialize_number_from_string<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNum {
        Num(u32),
        Str(String),
    }
    match StringOrNum::deserialize(deserializer)? {
        StringOrNum::Num(n) => Ok(n),
        StringOrNum::Str(s) => s.parse::<u32>().map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestAction {
    pub action_date: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillDetailResponse {
    pub bill: BillDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillDetail {
    #[serde(default, deserialize_with = "deserialize_number_or_string")]
    pub number: String,
    pub update_date: Option<String>,
    pub update_date_including_text: Option<String>,
    pub origin_chamber: Option<String>,
    pub origin_chamber_code: Option<String>,
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub introduced_date: Option<String>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    pub congress: u32,
    #[serde(default)]
    pub title: String,
    pub constitutional_authority_statement_text: Option<String>,
    pub policy_area: Option<PolicyArea>,
    #[serde(default)]
    pub sponsors: Vec<Sponsor>,
    pub latest_action: Option<LatestAction>,
    #[serde(default)]
    pub laws: Vec<Law>,
    #[serde(default)]
    pub cosponsors: Option<serde_json::Value>,
    pub cb_o_cost_estimates: Option<serde_json::Value>,
    #[serde(default)]
    pub committees: Option<serde_json::Value>,
    #[serde(default)]
    pub related_bills: Option<serde_json::Value>,
    #[serde(default)]
    pub actions: Option<serde_json::Value>,
    #[serde(default)]
    pub summaries: Option<serde_json::Value>,
    #[serde(default)]
    pub text_versions: Option<serde_json::Value>,
    #[serde(default)]
    pub amendments: Option<serde_json::Value>,
    #[serde(default)]
    pub subjects: Option<serde_json::Value>,
    #[serde(default)]
    pub titles: Option<serde_json::Value>,
}

/// Deserialize a value that may arrive as a JSON number or string, always producing a String.
fn deserialize_number_or_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrStr {
        Num(u64),
        Str(String),
    }
    match NumOrStr::deserialize(deserializer)? {
        NumOrStr::Num(n) => Ok(n.to_string()),
        NumOrStr::Str(s) => Ok(s),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyArea {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sponsor {
    pub bioguide_id: Option<String>,
    pub full_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub party: Option<String>,
    pub state: Option<String>,
    pub district: Option<u32>,
    pub is_by_request: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Law {
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextVersionsResponse {
    #[serde(rename = "textVersions", default)]
    pub text_versions: Vec<TextVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextVersion {
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub date: Option<String>,
    #[serde(default)]
    pub formats: Vec<TextFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFormat {
    pub url: String,
    #[serde(rename = "type")]
    pub r#type: Option<String>,
}

// ---------------------------------------------------------------------------
// CongressClient methods
// ---------------------------------------------------------------------------

impl CongressClient {
    /// List bills for a given congress and bill type with pagination.
    ///
    /// Calls `GET /bill/{congress}/{type}?offset={offset}&limit={limit}`.
    pub async fn list_bills(
        &self,
        congress: Congress,
        bill_type: BillType,
        offset: u32,
        limit: u32,
    ) -> Result<BillListResponse, ApiError> {
        let path = format!("/bill/{}/{}", congress.number(), bill_type.api_slug());
        let offset_s = offset.to_string();
        let limit_s = limit.to_string();
        let params: Vec<(&str, &str)> = vec![("offset", &offset_s), ("limit", &limit_s)];
        self.get::<BillListResponse>(&path, &params).await
    }

    /// Fetch full details for a single bill.
    ///
    /// Calls `GET /bill/{congress}/{type}/{number}`.
    pub async fn get_bill(&self, id: &BillId) -> Result<BillDetail, ApiError> {
        let path = format!("/bill/{}", id.api_path());
        let resp: BillDetailResponse = self.get(&path, &[]).await?;
        Ok(resp.bill)
    }

    /// Fetch text versions available for a bill.
    ///
    /// Calls `GET /bill/{congress}/{type}/{number}/text`.
    pub async fn get_bill_text(&self, id: &BillId) -> Result<Vec<TextVersion>, ApiError> {
        let path = format!("/bill/{}/text", id.api_path());
        let resp: TextVersionsResponse = self.get(&path, &[]).await?;
        Ok(resp.text_versions)
    }

    /// Convenience method that fetches H.R.1 of the 118th Congress as a quick
    /// API connectivity / key-validity check.
    pub async fn test_api(&self) -> Result<BillDetail, ApiError> {
        let congress =
            Congress::try_from(118).map_err(|e| ApiError::InvalidInput(e.to_string()))?;
        let id = BillId {
            congress,
            bill_type: BillType::Hr,
            number: 1,
        };
        self.get_bill(&id).await
    }
}
