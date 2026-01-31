use serde::Deserialize;

use super::error::{JsonToHwpxError, Result};

/// 게시판 게시글 문서 구조 (JSON 입력 최상위)
#[derive(Debug, Deserialize)]
pub struct ArticleDocument {
    /// 스키마 버전 (예: "1.0")
    pub schema_version: Option<String>,
    /// 게시글 고유 ID (필수)
    pub article_id: String,
    /// 게시글 제목
    pub title: Option<String>,
    /// 메타데이터 (작성자, 날짜 등)
    #[serde(default)]
    pub metadata: Option<Metadata>,
    /// 본문 콘텐츠 배열
    #[serde(default)]
    pub contents: Vec<Content>,
    /// 원본 HTML (무시)
    pub content_html: Option<String>,
    /// 첨부파일 목록 (무시)
    pub attachments: Option<Vec<serde_json::Value>>,
    /// 첨부파일 개수 (무시)
    pub attachment_count: Option<u64>,
    /// 첨부파일 총 용량 (무시)
    pub total_attachment_size: Option<u64>,
}

impl ArticleDocument {
    /// 입력 데이터 검증
    ///
    /// - article_id 비어있지 않음 확인
    pub fn validate(&self) -> Result<()> {
        if self.article_id.trim().is_empty() {
            return Err(JsonToHwpxError::Input(
                "article_id가 비어있습니다".to_string(),
            ));
        }

        Ok(())
    }
}

/// 메타데이터 구조
#[derive(Debug, Deserialize, Default)]
pub struct Metadata {
    /// 작성자
    pub author: Option<String>,
    /// 작성일시 (ISO 8601)
    pub created_at: Option<String>,
    /// 수정일시 (ISO 8601)
    pub updated_at: Option<String>,
    /// 작성 부서
    pub department: Option<String>,
    /// 게시판 ID
    pub board_id: Option<String>,
    /// 게시판 이름
    pub board_name: Option<String>,
    /// 게시판 폴더 ID
    pub folder_id: Option<String>,
    /// 게시 만료일
    pub expiry: Option<String>,
    /// 조회수
    pub views: Option<u64>,
    /// 좋아요 수
    pub likes: Option<u64>,
    /// 댓글 수
    pub comments: Option<u64>,
}

/// 변환 옵션 (CLI/API 파라미터로 전달)
#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    /// 헤더 포함 여부
    pub include_header: bool,
    /// 포함할 헤더 필드 목록
    pub header_fields: Vec<String>,
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
            "schema_version": "1.0",
            "article_id": "TEST001",
            "title": "테스트",
            "contents": [
                { "type": "text", "value": "안녕하세요" }
            ]
        }"#;

        let doc: ArticleDocument = serde_json::from_str(json).unwrap();
        assert_eq!(doc.article_id, "TEST001");
        assert_eq!(doc.title.as_deref(), Some("테스트"));
        assert_eq!(doc.contents.len(), 1);
    }

    #[test]
    fn test_validate_success() {
        let json = r#"{
            "article_id": "T1",
            "title": "S"
        }"#;

        let doc: ArticleDocument = serde_json::from_str(json).unwrap();
        assert!(doc.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_article_id() {
        let json = r#"{
            "article_id": "  ",
            "title": "S"
        }"#;

        let doc: ArticleDocument = serde_json::from_str(json).unwrap();
        let err = doc.validate().unwrap_err();
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn test_parse_image_url() {
        let json = r#"{
            "article_id": "T1",
            "contents": [
                { "type": "image", "url": "./test.png" }
            ]
        }"#;

        let doc: ArticleDocument = serde_json::from_str(json).unwrap();
        match &doc.contents[0] {
            Content::Image { url, .. } => assert_eq!(url.as_deref(), Some("./test.png")),
            _ => panic!("Expected Image content"),
        }
    }

    #[test]
    fn test_parse_empty_contents() {
        let json = r#"{
            "article_id": "T1",
            "contents": []
        }"#;

        let doc: ArticleDocument = serde_json::from_str(json).unwrap();
        assert!(doc.contents.is_empty());
    }

    #[test]
    fn test_invalid_json() {
        let json = r#"{ this is not valid json }"#;
        let result = serde_json::from_str::<ArticleDocument>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_content_type() {
        let json = r#"{
            "article_id": "T1",
            "contents": [
                { "type": "unknown", "value": "test" }
            ]
        }"#;

        let result = serde_json::from_str::<ArticleDocument>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_article_id() {
        let json = r#"{
            "title": "S",
            "contents": []
        }"#;

        let result = serde_json::from_str::<ArticleDocument>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_metadata() {
        let json = r#"{
            "article_id": "T1",
            "metadata": {
                "author": "홍길동",
                "department": "개발팀",
                "created_at": "2025-01-24T10:00:00+09:00"
            }
        }"#;

        let doc: ArticleDocument = serde_json::from_str(json).unwrap();
        let meta = doc.metadata.unwrap();
        assert_eq!(meta.author.as_deref(), Some("홍길동"));
        assert_eq!(meta.department.as_deref(), Some("개발팀"));
    }

    #[test]
    fn test_parse_with_ignored_fields() {
        let json = r#"{
            "schema_version": "1.0",
            "article_id": "T1",
            "title": "S",
            "content_html": "<p>original</p>",
            "attachments": [{"file_id": "F1"}],
            "attachment_count": 1,
            "total_attachment_size": 1024,
            "contents": []
        }"#;

        let doc: ArticleDocument = serde_json::from_str(json).unwrap();
        assert_eq!(doc.schema_version.as_deref(), Some("1.0"));
        assert!(doc.content_html.is_some());
        assert!(doc.attachments.is_some());
        assert_eq!(doc.attachment_count, Some(1));
    }

    #[test]
    fn test_convert_options_default() {
        let opts = ConvertOptions::default();
        assert!(!opts.include_header);
        assert!(opts.header_fields.is_empty());
    }
}
