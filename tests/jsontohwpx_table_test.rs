use std::path::PathBuf;

use hwpers::jsontohwpx::{self, ArticleDocument, ConvertOptions};
use hwpers::HwpxReader;

fn base_path() -> PathBuf {
    PathBuf::from("examples/jsontohwpx")
}

fn default_options() -> ConvertOptions {
    ConvertOptions::default()
}

fn verify_hwpx_bytes(bytes: &[u8]) {
    assert!(!bytes.is_empty());
    assert!(bytes[0..2] == [0x50, 0x4B], "유효한 ZIP 파일이 아닙니다");
    HwpxReader::from_bytes(bytes).expect("HwpxReader가 HWPX 파일을 읽지 못했습니다");
}

fn convert_example_file(filename: &str) -> hwpers::HwpDocument {
    let path = base_path().join(filename);
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("예제 파일 읽기 실패: {} ({})", path.display(), e));
    let input: ArticleDocument = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("JSON 파싱 실패: {} ({})", filename, e));
    let bytes = jsontohwpx::convert(&input, &default_options(), &base_path())
        .unwrap_or_else(|e| panic!("변환 실패: {} ({})", filename, e));
    verify_hwpx_bytes(&bytes);
    HwpxReader::from_bytes(&bytes).unwrap()
}

#[test]
fn test_with_table_example() {
    let doc = convert_example_file("with_table.json");
    let text = doc.extract_text();
    assert!(text.contains("1분기 실적"), "본문 텍스트 포함");
    assert!(text.contains("15% 성장"), "후행 텍스트 포함");
}

#[test]
fn test_table_merge_example() {
    let doc = convert_example_file("table_merge.json");
    let text = doc.extract_text();
    assert!(text.contains("셀 병합이 포함된"), "본문 텍스트 포함");
}

#[test]
fn test_table_merge1_colspan_only() {
    let _doc = convert_example_file("table_merge1.json");
}

#[test]
fn test_table_merge2_rowspan_only() {
    let _doc = convert_example_file("table_merge2.json");
}

#[test]
fn test_table_merge3_colspan_rowspan_block() {
    let _doc = convert_example_file("table_merge3.json");
}

#[test]
fn test_table_merge4_complex_multi_merge() {
    let _doc = convert_example_file("table_merge4.json");
}

#[test]
fn test_table_merge5_irregular_pattern() {
    let _doc = convert_example_file("table_merge5.json");
}

#[test]
fn test_table_merge_cell_structure() {
    let html = r#"<table>
        <tr><th colspan="3">제목</th></tr>
        <tr><td rowspan="2">그룹</td><td>A</td><td>1</td></tr>
        <tr><td>B</td><td>2</td></tr>
    </table>"#;

    let json = format!(
        r#"{{"article_id":"MERGE_STRUCT","title":"구조검증","contents":[{{"type":"table","value":"{}"}}]}}"#,
        html.replace('\n', "").replace('"', "\\\"").replace("    ", "")
    );

    let input: ArticleDocument = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &default_options(), &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut section_xml = String::new();
    {
        use std::io::Read;
        let mut file = archive.by_name("Contents/section0.xml").unwrap();
        file.read_to_string(&mut section_xml).unwrap();
    }

    assert!(section_xml.contains(r#"colAddr="0" rowAddr="0""#));
    assert!(section_xml.contains(r#"colSpan="3" rowSpan="1""#));
    assert!(section_xml.contains(r#"colAddr="0" rowAddr="1""#));
    assert!(section_xml.contains(r#"colSpan="1" rowSpan="2""#));
    assert!(section_xml.contains(r#"colAddr="1" rowAddr="2""#));
    assert!(section_xml.contains(r#"colAddr="2" rowAddr="2""#));
}

#[test]
fn test_table_merge_width_calculation() {
    let html = r#"<table>
        <tr><td colspan="2">병합</td><td>단일</td></tr>
        <tr><td>A</td><td>B</td><td>C</td></tr>
    </table>"#;

    let json = format!(
        r#"{{"article_id":"MERGE_WIDTH","title":"너비검증","contents":[{{"type":"table","value":"{}"}}]}}"#,
        html.replace('\n', "").replace('"', "\\\"").replace("    ", "")
    );

    let input: ArticleDocument = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &default_options(), &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut section_xml = String::new();
    {
        use std::io::Read;
        let mut file = archive.by_name("Contents/section0.xml").unwrap();
        file.read_to_string(&mut section_xml).unwrap();
    }

    assert!(section_xml.contains(r#"width="28346""#), "colspan=2 셀 너비");
    assert!(section_xml.contains(r#"width="14173""#), "단일 셀 너비");
}

#[test]
fn test_table_merge_height_calculation() {
    let html = r#"<table>
        <tr><td rowspan="3">병합</td><td>A</td></tr>
        <tr><td>B</td></tr>
        <tr><td>C</td></tr>
    </table>"#;

    let json = format!(
        r#"{{"article_id":"MERGE_HEIGHT","title":"높이검증","contents":[{{"type":"table","value":"{}"}}]}}"#,
        html.replace('\n', "").replace('"', "\\\"").replace("    ", "")
    );

    let input: ArticleDocument = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &default_options(), &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut section_xml = String::new();
    {
        use std::io::Read;
        let mut file = archive.by_name("Contents/section0.xml").unwrap();
        file.read_to_string(&mut section_xml).unwrap();
    }

    assert!(section_xml.contains(r#"height="3000""#), "rowspan=3 셀 높이");
    assert!(section_xml.contains(r#"height="1000""#), "일반 셀 높이");
}

#[test]
fn test_table_with_text_before_after() {
    let json = r#"{
        "article_id": "TBL_MIX001",
        "title": "혼합",
        "contents": [
            { "type": "text", "value": "표 앞 텍스트" },
            { "type": "table", "value": "<table><tr><td>셀</td></tr></table>" },
            { "type": "text", "value": "표 뒤 텍스트" }
        ]
    }"#;

    let input: ArticleDocument = serde_json::from_str(json).unwrap();
    let bytes = jsontohwpx::convert(&input, &default_options(), &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    let doc = HwpxReader::from_bytes(&bytes).unwrap();
    let text = doc.extract_text();
    assert!(text.contains("표 앞 텍스트"));
    assert!(text.contains("표 뒤 텍스트"));
}

#[test]
fn test_multiple_tables() {
    let json = r#"{
        "article_id": "TBL_MULTI001",
        "title": "다중 테이블",
        "contents": [
            { "type": "table", "value": "<table><tr><td>첫째 표</td></tr></table>" },
            { "type": "text", "value": "중간 텍스트" },
            { "type": "table", "value": "<table><tr><td>둘째 표</td></tr></table>" }
        ]
    }"#;

    let input: ArticleDocument = serde_json::from_str(json).unwrap();
    let bytes = jsontohwpx::convert(&input, &default_options(), &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);
}

#[test]
fn test_invalid_html_table() {
    let json = r#"{
        "article_id": "TBL_ERR001",
        "title": "에러",
        "contents": [
            { "type": "table", "value": "<table></table>" }
        ]
    }"#;

    let input: ArticleDocument = serde_json::from_str(json).unwrap();
    let result = jsontohwpx::convert(&input, &default_options(), &base_path());
    assert!(result.is_err(), "빈 테이블은 에러를 반환해야 함");
}

#[test]
fn test_large_table() {
    let mut rows_html = String::new();
    for i in 0..20 {
        rows_html.push_str(&format!(
            "<tr><td>행{}</td><td>값A{}</td><td>값B{}</td><td>값C{}</td></tr>",
            i, i, i, i
        ));
    }
    let html = format!(
        "<table><thead><tr><th>번호</th><th>A</th><th>B</th><th>C</th></tr></thead><tbody>{}</tbody></table>",
        rows_html
    );

    let json = format!(
        r#"{{"article_id":"TBL_BIG001","title":"큰 테이블","contents":[{{"type":"table","value":"{}"}}]}}"#,
        html.replace('"', "\\\"")
    );

    let input: ArticleDocument = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &default_options(), &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);
}
