use crate::hwpx::{HwpxTable, HwpxWriter};

use super::error::{JsonToHwpxError, Result};

/// HTML 테이블 문자열을 파싱하여 HwpxWriter에 추가
pub fn add_table_from_html(writer: &mut HwpxWriter, html: &str) -> Result<()> {
    let table = parse_html_table(html)?;
    writer.add_table(table)?;
    Ok(())
}

/// Parsed cell info from HTML
struct ParsedCell {
    text: String,
    col_span: u32,
    row_span: u32,
}

/// HTML <table> 태그를 파싱하여 HwpxTable 생성 (colspan/rowspan 지원)
fn parse_html_table(html: &str) -> Result<HwpxTable> {
    let document = scraper::Html::parse_fragment(html);
    let tr_selector = scraper::Selector::parse("tr")
        .map_err(|_| JsonToHwpxError::Conversion("tr 셀렉터 파싱 실패".to_string()))?;
    let cell_selector = scraper::Selector::parse("th, td")
        .map_err(|_| JsonToHwpxError::Conversion("th/td 셀렉터 파싱 실패".to_string()))?;

    // First pass: collect parsed cells per row
    let mut parsed_rows: Vec<Vec<ParsedCell>> = Vec::new();

    for tr in document.select(&tr_selector) {
        let mut row: Vec<ParsedCell> = Vec::new();
        for cell in tr.select(&cell_selector) {
            let text = cell.text().collect::<Vec<_>>().join("");
            let col_span = cell
                .value()
                .attr("colspan")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1)
                .max(1);
            let row_span = cell
                .value()
                .attr("rowspan")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1)
                .max(1);
            row.push(ParsedCell {
                text: text.trim().to_string(),
                col_span,
                row_span,
            });
        }
        if !row.is_empty() {
            parsed_rows.push(row);
        }
    }

    if parsed_rows.is_empty() {
        return Err(JsonToHwpxError::Conversion(
            "테이블에 행이 없습니다".to_string(),
        ));
    }

    // Determine grid dimensions
    // Calculate effective column count considering colspan
    let row_count = parsed_rows.len();
    let mut col_count: usize = 0;
    for parsed_row in &parsed_rows {
        let row_cols: usize = parsed_row.iter().map(|c| c.col_span as usize).sum();
        if row_cols > col_count {
            col_count = row_cols;
        }
    }
    // Also account for rowspan pushing cells into later rows
    // We'll use a grid-based approach to handle this correctly

    // Build the grid: track which positions are occupied
    // We may need to expand row_count if rowspan extends beyond parsed rows
    let mut max_row = row_count;
    for (r, parsed_row) in parsed_rows.iter().enumerate() {
        for cell in parsed_row {
            let end_row = r + cell.row_span as usize;
            if end_row > max_row {
                max_row = end_row;
            }
        }
    }

    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); col_count]; max_row];
    let mut occupied: Vec<Vec<bool>> = vec![vec![false; col_count]; max_row];
    let mut spans: Vec<(usize, usize, u32, u32)> = Vec::new(); // (row, col, col_span, row_span)

    for (row_idx, parsed_row) in parsed_rows.iter().enumerate() {
        let mut col_cursor: usize = 0;
        for cell in parsed_row {
            // Find next available column in this row
            while col_cursor < col_count && occupied[row_idx][col_cursor] {
                col_cursor += 1;
            }
            if col_cursor >= col_count {
                break;
            }

            // Place cell at (row_idx, col_cursor)
            grid[row_idx][col_cursor] = cell.text.clone();

            // Mark occupied cells and record span
            let cs = cell.col_span.min(col_count as u32 - col_cursor as u32);
            let rs = cell.row_span.min(max_row as u32 - row_idx as u32);

            if cs > 1 || rs > 1 {
                spans.push((row_idx, col_cursor, cs, rs));
            }

            for row in occupied.iter_mut().skip(row_idx).take(rs as usize) {
                for col in row.iter_mut().skip(col_cursor).take(cs as usize) {
                    *col = true;
                }
            }

            col_cursor += cs as usize;
        }
    }

    // Build HwpxTable
    let data: Vec<Vec<&str>> = grid
        .iter()
        .map(|r| r.iter().map(|s| s.as_str()).collect())
        .collect();
    let mut table = HwpxTable::from_data(data);

    for (row, col, cs, rs) in spans {
        table.set_cell_span(row, col, cs, rs);
    }

    Ok(table)
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
    fn test_colspan() {
        let html = r#"<table><tr><th colspan="3">합계</th></tr><tr><td>A</td><td>B</td><td>C</td></tr></table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].len(), 3);
        assert_eq!(table.rows[0][0], "합계");
        // colspan=3 span info
        let span = table.get_cell_span(0, 0);
        assert_eq!(span.col_span, 3);
        assert_eq!(span.row_span, 1);
        // Covered cells
        assert!(table.is_covered(0, 1));
        assert!(table.is_covered(0, 2));
        // Second row is normal
        assert_eq!(table.rows[1], vec!["A", "B", "C"]);
        assert!(!table.is_covered(1, 0));
    }

    #[test]
    fn test_rowspan() {
        let html =
            r#"<table><tr><td rowspan="2">병합</td><td>A</td></tr><tr><td>B</td></tr></table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0][0], "병합");
        assert_eq!(table.rows[0][1], "A");
        // rowspan=2 span info
        let span = table.get_cell_span(0, 0);
        assert_eq!(span.col_span, 1);
        assert_eq!(span.row_span, 2);
        // Cell (1,0) is covered by rowspan
        assert!(table.is_covered(1, 0));
        // "B" goes to col 1 in row 1
        assert_eq!(table.rows[1][1], "B");
    }

    #[test]
    fn test_colspan_and_rowspan_combined() {
        let html = r#"<table>
            <tr><td colspan="2" rowspan="2">병합</td><td>C</td></tr>
            <tr><td>F</td></tr>
            <tr><td>G</td><td>H</td><td>I</td></tr>
        </table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 3);
        assert_eq!(table.rows[0][0], "병합");
        let span = table.get_cell_span(0, 0);
        assert_eq!(span.col_span, 2);
        assert_eq!(span.row_span, 2);
        // Covered: (0,1), (1,0), (1,1)
        assert!(table.is_covered(0, 1));
        assert!(table.is_covered(1, 0));
        assert!(table.is_covered(1, 1));
        // "C" at (0,2), "F" at (1,2)
        assert_eq!(table.rows[0][2], "C");
        assert_eq!(table.rows[1][2], "F");
        // Row 2 is normal
        assert_eq!(table.rows[2], vec!["G", "H", "I"]);
    }

    #[test]
    fn test_multiple_colspan_in_one_row() {
        // 한 행에 여러 colspan이 있는 경우
        let html = r#"<table>
            <tr><td colspan="2">AB</td><td colspan="2">CD</td></tr>
            <tr><td>A</td><td>B</td><td>C</td><td>D</td></tr>
        </table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows[0].len(), 4);
        assert_eq!(table.rows[0][0], "AB");
        assert_eq!(table.rows[0][2], "CD");
        assert!(table.is_covered(0, 1));
        assert!(table.is_covered(0, 3));
        let s1 = table.get_cell_span(0, 0);
        assert_eq!(s1.col_span, 2);
        let s2 = table.get_cell_span(0, 2);
        assert_eq!(s2.col_span, 2);
    }

    #[test]
    fn test_multiple_rowspan_in_one_column() {
        // 한 열에 연속적인 rowspan이 있는 경우
        let html = r#"<table>
            <tr><td rowspan="2">G1</td><td>A</td></tr>
            <tr><td>B</td></tr>
            <tr><td rowspan="2">G2</td><td>C</td></tr>
            <tr><td>D</td></tr>
        </table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 4);
        assert_eq!(table.rows[0][0], "G1");
        assert_eq!(table.rows[2][0], "G2");
        assert!(table.is_covered(1, 0));
        assert!(table.is_covered(3, 0));
        assert_eq!(table.rows[1][1], "B");
        assert_eq!(table.rows[3][1], "D");
    }

    #[test]
    fn test_rowspan_3_with_adjacent_cells() {
        // rowspan=3이 있을 때 인접 셀이 올바르게 배치되는지
        let html = r#"<table>
            <tr><td rowspan="3">개발본부</td><td>프론트</td><td>5</td></tr>
            <tr><td>백엔드</td><td>8</td></tr>
            <tr><td>인프라</td><td>3</td></tr>
        </table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 3);
        assert_eq!(table.rows[0][0], "개발본부");
        let span = table.get_cell_span(0, 0);
        assert_eq!(span.row_span, 3);
        assert!(table.is_covered(1, 0));
        assert!(table.is_covered(2, 0));
        assert_eq!(table.rows[1][1], "백엔드");
        assert_eq!(table.rows[1][2], "8");
        assert_eq!(table.rows[2][1], "인프라");
        assert_eq!(table.rows[2][2], "3");
    }

    #[test]
    fn test_complex_irregular_merge() {
        // 불규칙한 패턴: 여러 위치에 다양한 span
        let html = r#"<table>
            <tr><td colspan="3">헤더</td><td colspan="2">서브</td></tr>
            <tr><td rowspan="2">좌</td><td>A</td><td>B</td><td rowspan="2" colspan="2">큰영역</td></tr>
            <tr><td colspan="2">중간</td></tr>
            <tr><td colspan="5">하단</td></tr>
        </table>"#;
        let table = parse_html_table(html).unwrap();
        assert_eq!(table.rows.len(), 4);
        assert_eq!(table.rows[0].len(), 5);

        // Row 0: 헤더(cs3) + 서브(cs2)
        assert_eq!(table.rows[0][0], "헤더");
        assert_eq!(table.get_cell_span(0, 0).col_span, 3);
        assert_eq!(table.rows[0][3], "서브");
        assert_eq!(table.get_cell_span(0, 3).col_span, 2);

        // Row 1: 좌(rs2) + A + B + 큰영역(rs2,cs2)
        assert_eq!(table.rows[1][0], "좌");
        assert_eq!(table.get_cell_span(1, 0).row_span, 2);
        assert_eq!(table.rows[1][3], "큰영역");
        assert_eq!(table.get_cell_span(1, 3).col_span, 2);
        assert_eq!(table.get_cell_span(1, 3).row_span, 2);

        // Row 2: col 0 covered, 중간(cs2), cols 3-4 covered
        assert!(table.is_covered(2, 0));
        assert_eq!(table.rows[2][1], "중간");
        assert_eq!(table.get_cell_span(2, 1).col_span, 2);
        assert!(table.is_covered(2, 3));
        assert!(table.is_covered(2, 4));

        // Row 3: 하단(cs5)
        assert_eq!(table.rows[3][0], "하단");
        assert_eq!(table.get_cell_span(3, 0).col_span, 5);
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
