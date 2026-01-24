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
}
