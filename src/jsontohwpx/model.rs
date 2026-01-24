use serde::Deserialize;

use super::error::{JsonToHwpxError, Result};

/// API 응답 최상위 구조
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse {
    pub response_code: String,
    pub response_text: Option<String>,
    #[serde(default)]
    pub options: Options,
    pub data: Data,
}

impl ApiResponse {
    /// responseCode가 "0"인지 검증
    pub fn validate(&self) -> Result<()> {
        if self.response_code != "0" {
            return Err(JsonToHwpxError::Input(format!(
                "responseCode가 '0'이 아닙니다: code='{}', text='{}'",
                self.response_code,
                self.response_text.as_deref().unwrap_or("")
            )));
        }
        Ok(())
    }
}

/// 변환 옵션
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    #[serde(default)]
    pub include_header: bool,
    #[serde(default)]
    pub header_fields: Vec<String>,
}

/// data 필드
#[derive(Debug, Deserialize)]
pub struct Data {
    pub article: Article,
}

/// article 구조 (메타데이터 + 본문)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    pub atcl_id: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub contents: Vec<Content>,
    #[serde(default)]
    pub reg_dt: Option<String>,
    #[serde(default)]
    pub reg_emp_name: Option<String>,
    #[serde(default)]
    pub reg_dept_name: Option<String>,
}

/// contents 배열의 각 요소
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    #[serde(rename = "text")]
    Text { value: String },
    #[serde(rename = "image")]
    Image {
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        base64: Option<String>,
        #[serde(default)]
        format: Option<String>,
    },
    #[serde(rename = "table")]
    Table { value: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_text() {
        let json = r#"{
            "responseCode": "0",
            "responseText": "SUCCESS",
            "data": {
                "article": {
                    "atclId": "TEST001",
                    "subject": "테스트",
                    "contents": [
                        { "type": "text", "value": "안녕하세요" }
                    ]
                }
            }
        }"#;

        let response: ApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.response_code, "0");
        assert_eq!(response.data.article.atcl_id, "TEST001");
        assert_eq!(response.data.article.contents.len(), 1);
    }

    #[test]
    fn test_validate_success() {
        let json = r#"{
            "responseCode": "0",
            "responseText": "SUCCESS",
            "data": { "article": { "atclId": "T1", "subject": "S" } }
        }"#;

        let response: ApiResponse = serde_json::from_str(json).unwrap();
        assert!(response.validate().is_ok());
    }

    #[test]
    fn test_validate_failure() {
        let json = r#"{
            "responseCode": "999",
            "responseText": "FAIL",
            "data": { "article": { "atclId": "T1", "subject": "S" } }
        }"#;

        let response: ApiResponse = serde_json::from_str(json).unwrap();
        let err = response.validate().unwrap_err();
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn test_parse_image_url() {
        let json = r#"{
            "responseCode": "0",
            "data": {
                "article": {
                    "atclId": "T1",
                    "subject": "S",
                    "contents": [
                        { "type": "image", "url": "./test.png" }
                    ]
                }
            }
        }"#;

        let response: ApiResponse = serde_json::from_str(json).unwrap();
        match &response.data.article.contents[0] {
            Content::Image { url, .. } => assert_eq!(url.as_deref(), Some("./test.png")),
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn test_parse_empty_contents() {
        let json = r#"{
            "responseCode": "0",
            "data": {
                "article": {
                    "atclId": "T1",
                    "subject": "S",
                    "contents": []
                }
            }
        }"#;

        let response: ApiResponse = serde_json::from_str(json).unwrap();
        assert!(response.data.article.contents.is_empty());
    }

    #[test]
    fn test_parse_options() {
        let json = r#"{
            "responseCode": "0",
            "options": {
                "includeHeader": true,
                "headerFields": ["subject", "regEmpName"]
            },
            "data": {
                "article": { "atclId": "T1", "subject": "S" }
            }
        }"#;

        let response: ApiResponse = serde_json::from_str(json).unwrap();
        assert!(response.options.include_header);
        assert_eq!(response.options.header_fields.len(), 2);
    }
}
