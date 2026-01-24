use std::path::PathBuf;

use hwpers::jsontohwpx::{self, ApiResponse};
use hwpers::HwpxReader;

fn base_path() -> PathBuf {
    PathBuf::from("examples/jsontohwpx")
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
    let input: ApiResponse = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("JSON 파싱 실패: {} ({})", filename, e));
    let bytes = jsontohwpx::convert(&input, &base_path())
        .unwrap_or_else(|e| panic!("변환 실패: {} ({})", filename, e));
    verify_hwpx_bytes(&bytes);
    HwpxReader::from_bytes(&bytes).unwrap()
}

#[test]
fn test_with_table_example() {
    let doc = convert_example_file("with_table.json");
    let text = doc.extract_text();
    // 테이블 외 텍스트 확인 (HwpxReader의 extract_text()는 테이블 셀 미포함)
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
    // colspan만 있는 테이블 (4열)
    let _doc = convert_example_file("table_merge1.json");
}

#[test]
fn test_table_merge2_rowspan_only() {
    // rowspan만 있는 테이블 (3열)
    let _doc = convert_example_file("table_merge2.json");
}

#[test]
fn test_table_merge3_colspan_rowspan_block() {
    // colspan+rowspan 큰 블록 머지 (4열)
    let _doc = convert_example_file("table_merge3.json");
}

#[test]
fn test_table_merge4_complex_multi_merge() {
    // 복잡한 다중 머지 (5열)
    let _doc = convert_example_file("table_merge4.json");
}

#[test]
fn test_table_merge5_irregular_pattern() {
    // 불규칙 머지 패턴 (5열)
    let _doc = convert_example_file("table_merge5.json");
}

#[test]
fn test_table_merge_cell_structure() {
    // 직접 HTML을 파싱하여 셀 구조 검증
    let html = r#"<table>
        <tr><th colspan="3">제목</th></tr>
        <tr><td rowspan="2">그룹</td><td>A</td><td>1</td></tr>
        <tr><td>B</td><td>2</td></tr>
    </table>"#;

    let json = format!(
        r#"{{"responseCode":"0","data":{{"article":{{"atclId":"MERGE_STRUCT","subject":"구조검증","contents":[{{"type":"table","value":"{}"}}]}}}}}}"#,
        html.replace('\n', "").replace('"', "\\\"").replace("    ", "")
    );

    let input: ApiResponse = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    // ZIP에서 section0.xml 추출하여 구조 확인
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut section_xml = String::new();
    {
        use std::io::Read;
        let mut file = archive.by_name("Contents/section0.xml").unwrap();
        file.read_to_string(&mut section_xml).unwrap();
    }

    // Row 0: colspan=3
    assert!(section_xml.contains(r#"colAddr="0" rowAddr="0""#));
    assert!(section_xml.contains(r#"colSpan="3" rowSpan="1""#));
    // Row 1: "그룹" rowspan=2
    assert!(section_xml.contains(r#"colAddr="0" rowAddr="1""#));
    assert!(section_xml.contains(r#"colSpan="1" rowSpan="2""#));
    // Row 2: col 0 is covered, starts at col 1
    assert!(section_xml.contains(r#"colAddr="1" rowAddr="2""#));
    // Covered cells should NOT have their own tc element at (0,2)
    // "B" is at colAddr=1, "2" is at colAddr=2 in row 2
    assert!(section_xml.contains(r#"colAddr="2" rowAddr="2""#));
}

#[test]
fn test_table_merge_width_calculation() {
    // 셀 너비가 올바르게 계산되는지 확인
    let html = r#"<table>
        <tr><td colspan="2">병합</td><td>단일</td></tr>
        <tr><td>A</td><td>B</td><td>C</td></tr>
    </table>"#;

    let json = format!(
        r#"{{"responseCode":"0","data":{{"article":{{"atclId":"MERGE_WIDTH","subject":"너비검증","contents":[{{"type":"table","value":"{}"}}]}}}}}}"#,
        html.replace('\n', "").replace('"', "\\\"").replace("    ", "")
    );

    let input: ApiResponse = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut section_xml = String::new();
    {
        use std::io::Read;
        let mut file = archive.by_name("Contents/section0.xml").unwrap();
        file.read_to_string(&mut section_xml).unwrap();
    }

    // 3열 테이블: col_width = 42520/3 = 14173
    // colspan=2 셀: width = 14173*2 = 28346
    assert!(section_xml.contains(r#"width="28346""#), "colspan=2 셀 너비");
    // 단일 셀: width = 14173
    assert!(section_xml.contains(r#"width="14173""#), "단일 셀 너비");
}

#[test]
fn test_table_merge_height_calculation() {
    // rowspan 시 높이가 올바르게 계산되는지 확인
    let html = r#"<table>
        <tr><td rowspan="3">병합</td><td>A</td></tr>
        <tr><td>B</td></tr>
        <tr><td>C</td></tr>
    </table>"#;

    let json = format!(
        r#"{{"responseCode":"0","data":{{"article":{{"atclId":"MERGE_HEIGHT","subject":"높이검증","contents":[{{"type":"table","value":"{}"}}]}}}}}}"#,
        html.replace('\n', "").replace('"', "\\\"").replace("    ", "")
    );

    let input: ApiResponse = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut section_xml = String::new();
    {
        use std::io::Read;
        let mut file = archive.by_name("Contents/section0.xml").unwrap();
        file.read_to_string(&mut section_xml).unwrap();
    }

    // rowspan=3: height = 1000*3 = 3000
    assert!(section_xml.contains(r#"height="3000""#), "rowspan=3 셀 높이");
    // 일반 셀: height = 1000
    assert!(section_xml.contains(r#"height="1000""#), "일반 셀 높이");
}

#[test]
fn test_table_with_text_before_after() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TBL_MIX001",
                "subject": "혼합",
                "contents": [
                    { "type": "text", "value": "표 앞 텍스트" },
                    { "type": "table", "value": "<table><tr><td>셀</td></tr></table>" },
                    { "type": "text", "value": "표 뒤 텍스트" }
                ]
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);

    let doc = HwpxReader::from_bytes(&bytes).unwrap();
    let text = doc.extract_text();
    assert!(text.contains("표 앞 텍스트"));
    assert!(text.contains("표 뒤 텍스트"));
}

#[test]
fn test_multiple_tables() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TBL_MULTI001",
                "subject": "다중 테이블",
                "contents": [
                    { "type": "table", "value": "<table><tr><td>첫째 표</td></tr></table>" },
                    { "type": "text", "value": "중간 텍스트" },
                    { "type": "table", "value": "<table><tr><td>둘째 표</td></tr></table>" }
                ]
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);
}

#[test]
fn test_invalid_html_table() {
    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TBL_ERR001",
                "subject": "에러",
                "contents": [
                    { "type": "table", "value": "<table></table>" }
                ]
            }
        }
    }"#;

    let input: ApiResponse = serde_json::from_str(json).unwrap();
    let result = jsontohwpx::convert(&input, &base_path());
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
        r#"{{"responseCode":"0","data":{{"article":{{"atclId":"TBL_BIG001","subject":"큰 테이블","contents":[{{"type":"table","value":"{}"}}]}}}}}}"#,
        html.replace('"', "\\\"")
    );

    let input: ApiResponse = serde_json::from_str(&json).unwrap();
    let bytes = jsontohwpx::convert(&input, &base_path()).unwrap();
    verify_hwpx_bytes(&bytes);
}
