use crate::hwpx::HwpxWriter;

use super::error::Result;

/// 텍스트 value를 \n 기준으로 분리하여 단락으로 추가
///
/// - `\n` = 새 단락 생성
/// - `\n\n` = 빈 단락 포함 (빈 줄 추가)
pub fn add_text_paragraphs(writer: &mut HwpxWriter, value: &str) -> Result<()> {
    let lines: Vec<&str> = value.split('\n').collect();

    for line in &lines {
        writer.add_paragraph(line)?;
    }

    Ok(())
}

/// 연속된 text 요소 사이 빈 단락 추가
pub fn add_separator_paragraph(writer: &mut HwpxWriter) -> Result<()> {
    writer.add_paragraph("")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_line() {
        let mut writer = HwpxWriter::new();
        add_text_paragraphs(&mut writer, "안녕하세요").unwrap();

        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_multiple_lines() {
        let mut writer = HwpxWriter::new();
        add_text_paragraphs(&mut writer, "첫 줄\n둘째 줄\n셋째 줄").unwrap();

        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_empty_line_between() {
        let mut writer = HwpxWriter::new();
        add_text_paragraphs(&mut writer, "첫 단락\n\n셋째 단락").unwrap();

        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_separator_paragraph() {
        let mut writer = HwpxWriter::new();
        add_text_paragraphs(&mut writer, "첫 텍스트").unwrap();
        add_separator_paragraph(&mut writer).unwrap();
        add_text_paragraphs(&mut writer, "둘째 텍스트").unwrap();

        let bytes = writer.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }
}
