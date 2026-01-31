use std::path::Path;

use crate::hwpx::{HwpxMetadata, HwpxTextStyle, HwpxWriter, StyledText};

use super::error::Result;
use super::image;
use super::model::{ArticleDocument, Content, ConvertOptions};
use super::table;
use super::text;

/// ArticleDocument를 HWPX 바이트로 변환
pub fn convert(
    input: &ArticleDocument,
    options: &ConvertOptions,
    base_path: &Path,
) -> Result<Vec<u8>> {
    input.validate()?;

    let mut writer = HwpxWriter::new();

    // 문서 메타데이터 설정
    let meta = input.metadata.as_ref();
    let creator = match (
        meta.and_then(|m| m.author.as_deref()),
        meta.and_then(|m| m.department.as_deref()),
    ) {
        (Some(name), Some(dept)) => format!("{} ({})", name, dept),
        (Some(name), None) => name.to_string(),
        _ => String::new(),
    };
    writer.set_metadata(HwpxMetadata {
        title: input.title.clone().unwrap_or_default(),
        creator,
        created_date: meta
            .and_then(|m| m.created_at.clone())
            .unwrap_or_default(),
    });

    // includeHeader 옵션 처리
    if options.include_header {
        add_header_section(&mut writer, input, options)?;
    }

    // 빈 contents 경고
    if input.contents.is_empty() {
        eprintln!("[경고] contents가 비어있습니다. 빈 문서를 생성합니다.");
    }

    // contents 순회하며 변환
    let mut has_prev = false;

    for content in &input.contents {
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

/// ArticleDocument를 HWPX 파일로 변환하여 저장
pub fn convert_to_file(
    input: &ArticleDocument,
    options: &ConvertOptions,
    base_path: &Path,
    output: &Path,
) -> Result<()> {
    let bytes = convert(input, options, base_path)?;
    std::fs::write(output, bytes)?;
    Ok(())
}

/// includeHeader 옵션에 따라 메타데이터를 본문 상단에 삽입
fn add_header_section(
    writer: &mut HwpxWriter,
    input: &ArticleDocument,
    options: &ConvertOptions,
) -> Result<()> {
    let meta = input.metadata.as_ref();
    let fields = &options.header_fields;

    let bold_style = HwpxTextStyle::new().bold();

    let field_entries: Vec<(&str, Option<&str>)> = vec![
        ("title", input.title.as_deref()),
        ("subject", input.title.as_deref()),
        ("author", meta.and_then(|m| m.author.as_deref())),
        ("regEmpName", meta.and_then(|m| m.author.as_deref())),
        ("department", meta.and_then(|m| m.department.as_deref())),
        ("regDeptName", meta.and_then(|m| m.department.as_deref())),
        ("created_at", meta.and_then(|m| m.created_at.as_deref())),
        ("regDt", meta.and_then(|m| m.created_at.as_deref())),
        ("board_name", meta.and_then(|m| m.board_name.as_deref())),
        ("expiry", meta.and_then(|m| m.expiry.as_deref())),
    ];

    let labels = [
        ("title", "제목"),
        ("subject", "제목"),
        ("author", "작성자"),
        ("regEmpName", "작성자"),
        ("department", "부서"),
        ("regDeptName", "부서"),
        ("created_at", "작성일"),
        ("regDt", "작성일"),
        ("board_name", "게시판"),
        ("expiry", "보존기간"),
    ];

    // 이미 출력한 라벨을 추적하여 중복 출력 방지
    let mut printed_labels = Vec::new();

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

            // 같은 라벨이 이미 출력되었으면 건너뜀 (title/subject, author/regEmpName 등)
            if printed_labels.contains(&label) {
                continue;
            }
            printed_labels.push(label);

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
    use crate::jsontohwpx::model::ArticleDocument;
    use std::path::PathBuf;

    fn base_path() -> PathBuf {
        PathBuf::from("examples/jsontohwpx")
    }

    fn default_options() -> ConvertOptions {
        ConvertOptions::default()
    }

    #[test]
    fn test_convert_simple_text() {
        let json = r#"{
            "article_id": "TEST001",
            "title": "테스트 문서",
            "contents": [
                { "type": "text", "value": "안녕하세요" }
            ]
        }"#;

        let input: ArticleDocument = serde_json::from_str(json).unwrap();
        let result = convert(&input, &default_options(), &base_path());
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_convert_empty_contents() {
        let json = r#"{
            "article_id": "TEST002",
            "title": "빈 문서",
            "contents": []
        }"#;

        let input: ArticleDocument = serde_json::from_str(json).unwrap();
        let result = convert(&input, &default_options(), &base_path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_empty_article_id_fails() {
        let json = r#"{
            "article_id": "  ",
            "title": "에러"
        }"#;

        let input: ArticleDocument = serde_json::from_str(json).unwrap();
        let result = convert(&input, &default_options(), &base_path());
        assert!(result.is_err());
    }
}
