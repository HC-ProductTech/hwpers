use crate::hwpx::{HwpxTable, HwpxWriter};

use super::error::{JsonToHwpxError, Result};

/// HTML 테이블 문자열을 파싱하여 HwpxWriter에 추가
pub fn add_table_from_html(writer: &mut HwpxWriter, html: &str) -> Result<()> {
    let table = parse_html_table(html)?;
    writer.add_table(table)?;
    Ok(())
}

/// HTML <table> 태그를 파싱하여 HwpxTable 생성
fn parse_html_table(html: &str) -> Result<HwpxTable> {
    let document = scraper::Html::parse_fragment(html);
    let tr_selector = scraper::Selector::parse("tr")
        .map_err(|_| JsonToHwpxError::Conversion("tr 셀렉터 파싱 실패".to_string()))?;
    let cell_selector = scraper::Selector::parse("th, td")
        .map_err(|_| JsonToHwpxError::Conversion("th/td 셀렉터 파싱 실패".to_string()))?;

    let mut rows: Vec<Vec<String>> = Vec::new();

    for tr in document.select(&tr_selector) {
        let mut row: Vec<String> = Vec::new();
        for cell in tr.select(&cell_selector) {
            let text = cell.text().collect::<Vec<_>>().join("");
            row.push(text.trim().to_string());
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }

    if rows.is_empty() {
        return Err(JsonToHwpxError::Conversion(
            "테이블에 행이 없습니다".to_string(),
        ));
    }

    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);

    // 열 수가 다른 행은 빈 셀로 패딩
    for row in &mut rows {
        while row.len() < col_count {
            row.push(String::new());
        }
    }

    let data: Vec<Vec<&str>> = rows
        .iter()
        .map(|r| r.iter().map(|s| s.as_str()).collect())
        .collect();

    Ok(HwpxTable::from_data(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_table() {
        let html = "<table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>";
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec!["A", "B"]);
        assert_eq!(table.rows[1], vec!["C", "D"]);
    }

    #[test]
    fn test_table_with_thead() {
        let html = "<table><thead><tr><th>헤더1</th><th>헤더2</th></tr></thead><tbody><tr><td>값1</td><td>값2</td></tr></tbody></table>";
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec!["헤더1", "헤더2"]);
    }

    #[test]
    fn test_empty_table() {
        let html = "<table></table>";
        let result = parse_html_table(html);
        assert!(result.is_err());
    }

    #[test]
    fn test_uneven_columns() {
        let html = "<table><tr><td>A</td><td>B</td><td>C</td></tr><tr><td>D</td></tr></table>";
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows[1].len(), 3);
        assert_eq!(table.rows[1][1], "");
    }

    #[test]
    fn test_add_table_to_writer() {
        let mut writer = HwpxWriter::new();
        let html = "<table><tr><td>A</td><td>B</td></tr></table>";
        add_table_from_html(&mut writer, html).unwrap();

        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_colspan_fallback() {
        // colspan은 무시하고 텍스트만 추출
        let html = r#"<table><tr><th colspan="3">합계</th></tr><tr><td>A</td><td>B</td><td>C</td></tr></table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 2);
        // colspan=3인 셀은 1개 셀로 추출, 나머지는 빈 문자열로 패딩
        assert_eq!(table.rows[0][0], "합계");
        assert_eq!(table.rows[0].len(), 3); // 패딩됨
    }

    #[test]
    fn test_rowspan_fallback() {
        // rowspan은 무시, 각 행에서 보이는 셀만 추출
        let html =
            r#"<table><tr><td rowspan="2">병합</td><td>A</td></tr><tr><td>B</td></tr></table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0][0], "병합");
        assert_eq!(table.rows[0][1], "A");
        // 두 번째 행에는 "B"만 있고 패딩됨
        assert_eq!(table.rows[1][0], "B");
    }

    #[test]
    fn test_inline_style_ignored() {
        // 인라인 스타일은 무시하고 텍스트만 추출
        let html = r#"<table><tr><td style="color:red; font-weight:bold;">스타일</td><td class="highlight">클래스</td></tr></table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows[0][0], "스타일");
        assert_eq!(table.rows[0][1], "클래스");
    }

    #[test]
    fn test_th_treated_as_text() {
        // th는 td와 동일하게 텍스트만 추출 (HwpxTable이 셀별 스타일 미지원)
        let html = "<table><tr><th>헤더</th></tr><tr><td>데이터</td></tr></table>";
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows[0][0], "헤더");
        assert_eq!(table.rows[1][0], "데이터");
    }

    #[test]
    fn test_whitespace_trimming() {
        let html = "<table><tr><td>  공백  </td><td>\n줄바꿈\n</td></tr></table>";
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows[0][0], "공백");
        assert_eq!(table.rows[0][1], "줄바꿈");
    }

    #[test]
    fn test_nested_html_elements_text_only() {
        // 셀 내부 HTML 태그는 무시하고 텍스트만 추출
        let html = "<table><tr><td><b>굵게</b> 일반</td><td><a href='#'>링크</a></td></tr></table>";
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows[0][0], "굵게 일반");
        assert_eq!(table.rows[0][1], "링크");
    }

    #[test]
    fn test_empty_cells() {
        let html = "<table><tr><td></td><td>값</td></tr><tr><td>A</td><td></td></tr></table>";
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows[0][0], "");
        assert_eq!(table.rows[0][1], "값");
        assert_eq!(table.rows[1][0], "A");
        assert_eq!(table.rows[1][1], "");
    }

    #[test]
    fn test_table_with_hwpx_reader_verification() {
        let mut writer = HwpxWriter::new();
        let html = "<table><thead><tr><th>이름</th><th>나이</th></tr></thead><tbody><tr><td>홍길동</td><td>30</td></tr></tbody></table>";
        add_table_from_html(&mut writer, html).unwrap();

        let bytes = writer.to_bytes().unwrap();
        // HwpxReader가 생성된 HWPX를 정상적으로 읽을 수 있는지 확인
        let _doc = crate::HwpxReader::from_bytes(&bytes).unwrap();
    }
}
