use std::path::Path;

use crate::hwpx::{HwpxMetadata, HwpxTextStyle, HwpxWriter, StyledText};

use super::error::Result;
use super::image;
use super::model::{ApiResponse, Content};
use super::table;
use super::text;

/// JSON ApiResponse를 HWPX 바이트로 변환
pub fn convert(input: &ApiResponse, base_path: &Path) -> Result<Vec<u8>> {
    input.validate()?;

    let mut writer = HwpxWriter::new();
    let article = &input.data.article;

    // 문서 메타데이터 설정
    let creator = match (&article.reg_emp_name, &article.reg_dept_name) {
        (Some(name), Some(dept)) => format!("{} ({})", name, dept),
        (Some(name), None) => name.clone(),
        _ => String::new(),
    };
    writer.set_metadata(HwpxMetadata {
        title: article.subject.clone(),
        creator,
        created_date: article.reg_dt.clone().unwrap_or_default(),
    });

    // includeHeader 옵션 처리
    if input.options.include_header {
        add_header_section(&mut writer, input)?;
    }

    // 빈 contents 경고
    if article.contents.is_empty() {
        eprintln!("[경고] contents가 비어있습니다. 빈 문서를 생성합니다.");
    }

    // contents 순회하며 변환
    let mut has_prev = false;

    for content in &article.contents {
        // 각 콘텐츠 항목 사이에 빈 단락(개행) 추가
        if has_prev {
            text::add_separator_paragraph(&mut writer)?;
        }

        match content {
            Content::Text { value } => {
                text::add_text_paragraphs(&mut writer, value)?;
            }
            Content::Image {
                url,
                base64,
                format,
            } => {
                if let Some(b64_data) = base64 {
                    image::add_image_from_base64(&mut writer, b64_data, format.as_deref())?;
                } else if let Some(url_str) = url {
                    image::add_image_from_url(&mut writer, url_str, base_path)?;
                }
            }
            Content::Table { value } => {
                table::add_table_from_html(&mut writer, value)?;
            }
        }
        has_prev = true;
    }

    let bytes = writer.to_bytes()?;
    Ok(bytes)
}

/// JSON ApiResponse를 HWPX 파일로 변환하여 저장
pub fn convert_to_file(input: &ApiResponse, base_path: &Path, output: &Path) -> Result<()> {
    let bytes = convert(input, base_path)?;
    std::fs::write(output, bytes)?;
    Ok(())
}

/// includeHeader 옵션에 따라 메타데이터를 본문 상단에 삽입
fn add_header_section(writer: &mut HwpxWriter, input: &ApiResponse) -> Result<()> {
    let article = &input.data.article;
    let fields = &input.options.header_fields;

    let bold_style = HwpxTextStyle::new().bold();

    let field_entries: Vec<(&str, Option<&str>)> = vec![
        ("subject", Some(article.subject.as_str())),
        ("regEmpName", article.reg_emp_name.as_deref()),
        ("regDeptName", article.reg_dept_name.as_deref()),
        ("regDt", article.reg_dt.as_deref()),
    ];

    let labels = [
        ("subject", "제목"),
        ("regEmpName", "작성자"),
        ("regDeptName", "부서"),
        ("regDt", "작성일"),
    ];

    for (field_key, value) in &field_entries {
        if !fields.is_empty() && !fields.iter().any(|f| f == *field_key) {
            continue;
        }

        if let Some(val) = value {
            let label = labels
                .iter()
                .find(|(k, _)| k == field_key)
                .map(|(_, l)| *l)
                .unwrap_or(*field_key);

            let runs = vec![
                StyledText::with_style(&format!("{}: ", label), bold_style.clone()),
                StyledText::new(val),
            ];
            writer.add_mixed_styled_paragraph(runs)?;
        }
    }

    // 구분선
    writer.add_paragraph("─────────────────────────")?;
    writer.add_paragraph("")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsontohwpx::model::ApiResponse;
    use std::path::PathBuf;

    fn base_path() -> PathBuf {
        PathBuf::from("examples/jsontohwpx")
    }

    #[test]
    fn test_convert_simple_text() {
        let json = r#"{
            "responseCode": "0",
            "responseText": "SUCCESS",
            "data": {
                "article": {
                    "atclId": "TEST001",
                    "subject": "테스트 문서",
                    "contents": [
                        { "type": "text", "value": "안녕하세요" }
                    ]
                }
            }
        }"#;

        let input: ApiResponse = serde_json::from_str(json).unwrap();
        let result = convert(&input, &base_path());
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_convert_empty_contents() {
        let json = r#"{
            "responseCode": "0",
            "data": {
                "article": {
                    "atclId": "TEST002",
                    "subject": "빈 문서",
                    "contents": []
                }
            }
        }"#;

        let input: ApiResponse = serde_json::from_str(json).unwrap();
        let result = convert(&input, &base_path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_invalid_response_code() {
        let json = r#"{
            "responseCode": "999",
            "responseText": "ERROR",
            "data": {
                "article": {
                    "atclId": "TEST003",
                    "subject": "에러"
                }
            }
        }"#;

        let input: ApiResponse = serde_json::from_str(json).unwrap();
        let result = convert(&input, &base_path());
        assert!(result.is_err());
    }
}
