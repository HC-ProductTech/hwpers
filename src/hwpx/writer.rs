use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::error::{HwpError, Result};
use crate::model::char_shape::CharShape;
use crate::model::para_char_shape::{CharPositionShape, ParaCharShape};
use crate::model::paragraph::{ParaText, Paragraph, Section};
use crate::model::HwpDocument;
use crate::parser::body_text::BodyText;
use crate::parser::doc_info::DocInfo;
use crate::parser::header::FileHeader;

// XML namespace declarations for HWPX 2011 format
const HWPX_NAMESPACES: &str = concat!(
    r#"xmlns:ha="http://www.hancom.co.kr/hwpml/2011/app" "#,
    r#"xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" "#,
    r#"xmlns:hp10="http://www.hancom.co.kr/hwpml/2016/paragraph" "#,
    r#"xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section" "#,
    r#"xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core" "#,
    r#"xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head" "#,
    r#"xmlns:hhs="http://www.hancom.co.kr/hwpml/2011/history" "#,
    r#"xmlns:hm="http://www.hancom.co.kr/hwpml/2011/master-page" "#,
    r#"xmlns:hpf="http://www.hancom.co.kr/schema/2011/hpf" "#,
    r#"xmlns:dc="http://purl.org/dc/elements/1.1/" "#,
    r#"xmlns:opf="http://www.idpf.org/2007/opf/" "#,
    r#"xmlns:ooxmlchart="http://www.hancom.co.kr/hwpml/2016/ooxmlchart" "#,
    r#"xmlns:hwpunitchar="http://www.hancom.co.kr/hwpml/2016/HwpUnitChar" "#,
    r#"xmlns:epub="http://www.idpf.org/2007/ops" "#,
    r#"xmlns:config="urn:oasis:names:tc:opendocument:xmlns:config:1.0""#
);

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Text style configuration for HWPX paragraphs
#[derive(Debug, Clone, Default)]
pub struct HwpxTextStyle {
    pub font_name: Option<String>,
    pub font_size: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub color: u32,
}

impl HwpxTextStyle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set font size in points
    pub fn size(mut self, size_pt: u32) -> Self {
        self.font_size = Some(size_pt);
        self
    }

    /// Set bold
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Set italic
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Set underline
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Set strikethrough
    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// Set text color (RGB format: 0xRRGGBB)
    pub fn color(mut self, color: u32) -> Self {
        self.color = color;
        self
    }

    /// Convert to CharShape for internal use
    fn to_char_shape(&self) -> CharShape {
        let mut properties = 0u32;

        if self.bold {
            properties |= 1 << 0; // Bit 0: Bold
        }
        if self.italic {
            properties |= 1 << 1; // Bit 1: Italic
        }
        if self.underline {
            properties |= 1 << 2; // Bit 2: Underline
        }
        if self.strikethrough {
            properties |= 1 << 3; // Bit 3: Strikethrough
        }

        let base_size = self.font_size.unwrap_or(10) as i32 * 100; // Convert pt to hwp units

        CharShape {
            face_name_ids: [0; 7],
            ratios: [100; 7],
            char_spaces: [0; 7],
            relative_sizes: [100; 7],
            char_offsets: [0; 7],
            base_size,
            properties,
            shadow_gap_x: 0,
            shadow_gap_y: 0,
            text_color: self.color,
            underline_color: self.color,
            shade_color: 0xFFFFFF,
            shadow_color: 0x808080,
            border_fill_id: 0,
        }
    }
}

/// A styled text run within a paragraph
#[derive(Debug, Clone)]
pub struct StyledText {
    pub text: String,
    pub style: HwpxTextStyle,
}

impl StyledText {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            style: HwpxTextStyle::default(),
        }
    }

    pub fn with_style(text: &str, style: HwpxTextStyle) -> Self {
        Self {
            text: text.to_string(),
            style,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HwpxImage {
    pub data: Vec<u8>,
    pub format: HwpxImageFormat,
    pub width_mm: Option<u32>,
    pub height_mm: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HwpxImageFormat {
    Png,
    Jpeg,
    Gif,
    Bmp,
}

impl HwpxImageFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
        }
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            Some(Self::Png)
        } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if data.starts_with(b"GIF") {
            Some(Self::Gif)
        } else if data.starts_with(b"BM") {
            Some(Self::Bmp)
        } else {
            None
        }
    }
}

impl HwpxImage {
    pub fn from_bytes(data: Vec<u8>) -> Option<Self> {
        let format = HwpxImageFormat::from_bytes(&data)?;
        let (width_mm, height_mm) = Self::read_dimensions_mm(&data, format);
        Some(Self {
            data,
            format,
            width_mm,
            height_mm,
        })
    }

    pub fn with_size(mut self, width_mm: u32, height_mm: u32) -> Self {
        self.width_mm = Some(width_mm);
        self.height_mm = Some(height_mm);
        self
    }

    /// 이미지 바이트에서 픽셀 크기를 읽고 mm로 변환 (96 DPI 기준)
    fn read_dimensions_mm(data: &[u8], format: HwpxImageFormat) -> (Option<u32>, Option<u32>) {
        let (w_px, h_px) = match format {
            HwpxImageFormat::Png => Self::read_png_dimensions(data),
            HwpxImageFormat::Jpeg => Self::read_jpeg_dimensions(data),
            HwpxImageFormat::Gif => Self::read_gif_dimensions(data),
            HwpxImageFormat::Bmp => Self::read_bmp_dimensions(data),
        };
        match (w_px, h_px) {
            (Some(w), Some(h)) if w > 0 && h > 0 => {
                // 96 DPI 기준으로 mm 변환
                let w_mm = (w as f64 * 25.4 / 96.0).round() as u32;
                let h_mm = (h as f64 * 25.4 / 96.0).round() as u32;
                (Some(w_mm.max(1)), Some(h_mm.max(1)))
            }
            _ => (None, None),
        }
    }

    fn read_png_dimensions(data: &[u8]) -> (Option<u32>, Option<u32>) {
        // PNG: 8-byte sig + 4-byte chunk_len + 4-byte "IHDR" + 4-byte width + 4-byte height
        if data.len() < 24 {
            return (None, None);
        }
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        (Some(w), Some(h))
    }

    fn read_jpeg_dimensions(data: &[u8]) -> (Option<u32>, Option<u32>) {
        // JPEG: SOF0(0xFFC0) 또는 SOF2(0xFFC2) 마커 찾기
        let mut i = 2;
        while i + 1 < data.len() {
            if data[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = data[i + 1];
            if marker == 0xC0 || marker == 0xC2 {
                if i + 9 < data.len() {
                    let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                    let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                    return (Some(w), Some(h));
                }
                return (None, None);
            }
            if i + 3 < data.len() {
                let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
                i += 2 + seg_len;
            } else {
                break;
            }
        }
        (None, None)
    }

    fn read_gif_dimensions(data: &[u8]) -> (Option<u32>, Option<u32>) {
        // GIF: 6-byte sig + 2-byte width(LE) + 2-byte height(LE)
        if data.len() < 10 {
            return (None, None);
        }
        let w = u16::from_le_bytes([data[6], data[7]]) as u32;
        let h = u16::from_le_bytes([data[8], data[9]]) as u32;
        (Some(w), Some(h))
    }

    fn read_bmp_dimensions(data: &[u8]) -> (Option<u32>, Option<u32>) {
        // BMP: 18-byte offset에 width(LE i32), 22-byte offset에 height(LE i32)
        if data.len() < 26 {
            return (None, None);
        }
        let w = i32::from_le_bytes([data[18], data[19], data[20], data[21]]).unsigned_abs();
        let h = i32::from_le_bytes([data[22], data[23], data[24], data[25]]).unsigned_abs();
        (Some(w), Some(h))
    }
}

/// Cell span information for merged cells
#[derive(Debug, Clone, Copy)]
pub struct CellSpan {
    pub col_span: u32,
    pub row_span: u32,
}

impl Default for CellSpan {
    fn default() -> Self {
        Self {
            col_span: 1,
            row_span: 1,
        }
    }
}

pub struct HwpxTable {
    /// Grid of cell text values indexed by logical (row, col) position.
    /// Only cells that are the "origin" of a merge have text;
    /// covered cells have empty strings and are skipped in output.
    pub rows: Vec<Vec<String>>,
    pub col_widths: Vec<u32>,
    /// Cell span info: key = (row, col) of the origin cell
    pub cell_spans: std::collections::HashMap<(usize, usize), CellSpan>,
    /// Tracks which cells are covered by another cell's span
    covered: std::collections::HashSet<(usize, usize)>,
}

impl HwpxTable {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows: vec![vec![String::new(); cols]; rows],
            col_widths: vec![8390; cols],
            cell_spans: std::collections::HashMap::new(),
            covered: std::collections::HashSet::new(),
        }
    }

    pub fn from_data(data: Vec<Vec<&str>>) -> Self {
        let rows: Vec<Vec<String>> = data
            .iter()
            .map(|row| row.iter().map(|s| s.to_string()).collect())
            .collect();
        let cols = rows.first().map(|r| r.len()).unwrap_or(0);
        Self {
            rows,
            col_widths: vec![8390; cols],
            cell_spans: std::collections::HashMap::new(),
            covered: std::collections::HashSet::new(),
        }
    }

    pub fn set_cell(&mut self, row: usize, col: usize, value: &str) {
        if row < self.rows.len() && col < self.rows[row].len() {
            self.rows[row][col] = value.to_string();
        }
    }

    /// Set cell merge span and mark covered cells
    pub fn set_cell_span(&mut self, row: usize, col: usize, col_span: u32, row_span: u32) {
        if col_span <= 1 && row_span <= 1 {
            return;
        }
        self.cell_spans.insert(
            (row, col),
            CellSpan { col_span, row_span },
        );
        // Mark all cells covered by this span (except the origin)
        for r in row..row + row_span as usize {
            for c in col..col + col_span as usize {
                if r != row || c != col {
                    self.covered.insert((r, c));
                }
            }
        }
    }

    /// Check if a cell position is covered by another cell's span
    pub fn is_covered(&self, row: usize, col: usize) -> bool {
        self.covered.contains(&(row, col))
    }

    /// Get the span for a cell (defaults to 1x1)
    pub fn get_cell_span(&self, row: usize, col: usize) -> CellSpan {
        self.cell_spans
            .get(&(row, col))
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub struct HwpxHyperlink {
    pub text: String,
    pub url: String,
}

impl HwpxHyperlink {
    pub fn new(text: &str, url: &str) -> Self {
        Self {
            text: text.to_string(),
            url: url.to_string(),
        }
    }
}

/// Header configuration for HWPX documents
#[derive(Debug, Clone)]
pub struct HwpxHeader {
    pub text: String,
    pub apply_to: HeaderFooterApplyTo,
}

impl HwpxHeader {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            apply_to: HeaderFooterApplyTo::All,
        }
    }

    pub fn for_odd_pages(text: &str) -> Self {
        Self {
            text: text.to_string(),
            apply_to: HeaderFooterApplyTo::Odd,
        }
    }

    pub fn for_even_pages(text: &str) -> Self {
        Self {
            text: text.to_string(),
            apply_to: HeaderFooterApplyTo::Even,
        }
    }
}

/// Footer configuration for HWPX documents
#[derive(Debug, Clone)]
pub struct HwpxFooter {
    pub text: String,
    pub include_page_number: bool,
    pub page_number_format: PageNumberFormat,
    pub apply_to: HeaderFooterApplyTo,
}

impl HwpxFooter {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            include_page_number: false,
            page_number_format: PageNumberFormat::Numeric,
            apply_to: HeaderFooterApplyTo::All,
        }
    }

    pub fn with_page_number(mut self) -> Self {
        self.include_page_number = true;
        self
    }

    pub fn with_page_number_format(mut self, format: PageNumberFormat) -> Self {
        self.include_page_number = true;
        self.page_number_format = format;
        self
    }

    pub fn for_odd_pages(mut self) -> Self {
        self.apply_to = HeaderFooterApplyTo::Odd;
        self
    }

    pub fn for_even_pages(mut self) -> Self {
        self.apply_to = HeaderFooterApplyTo::Even;
        self
    }
}

/// Page number format for footers
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageNumberFormat {
    /// 1, 2, 3, ...
    Numeric,
    /// i, ii, iii, ...
    RomanLower,
    /// I, II, III, ...
    RomanUpper,
    /// a, b, c, ...
    AlphaLower,
    /// A, B, C, ...
    AlphaUpper,
}

impl PageNumberFormat {
    fn as_hwpx_format(self) -> &'static str {
        match self {
            Self::Numeric => "DIGIT",
            Self::RomanLower => "ROMAN_SMALL",
            Self::RomanUpper => "ROMAN_CAPITAL",
            Self::AlphaLower => "LATIN_SMALL",
            Self::AlphaUpper => "LATIN_CAPITAL",
        }
    }
}

/// Which pages the header/footer applies to
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeaderFooterApplyTo {
    /// All pages
    All,
    /// Odd pages only
    Odd,
    /// Even pages only
    Even,
}

/// Document metadata for HWPX content.hpf
#[derive(Debug, Clone, Default)]
pub struct HwpxMetadata {
    pub title: String,
    pub creator: String,
    pub created_date: String,
}

pub struct HwpxWriter {
    document: HwpDocument,
    tables: Vec<(usize, HwpxTable)>,
    images: Vec<(usize, HwpxImage)>,
    hyperlinks: Vec<(usize, Vec<HwpxHyperlink>)>,
    headers: Vec<HwpxHeader>,
    footers: Vec<HwpxFooter>,
    next_table_id: u32,
    next_image_id: u32,
    metadata: HwpxMetadata,
}

impl HwpxWriter {
    pub fn new() -> Self {
        Self {
            document: HwpDocument {
                header: FileHeader::new_default(),
                doc_info: DocInfo::default(),
                body_texts: Vec::new(),
                preview_text: None,
                preview_image: None,
                summary_info: None,
            },
            tables: Vec::new(),
            images: Vec::new(),
            hyperlinks: Vec::new(),
            headers: Vec::new(),
            footers: Vec::new(),
            next_table_id: 1,
            next_image_id: 1,
            metadata: HwpxMetadata::default(),
        }
    }

    pub fn set_metadata(&mut self, metadata: HwpxMetadata) {
        self.metadata = metadata;
    }

    pub fn from_document(document: HwpDocument) -> Self {
        Self {
            document,
            tables: Vec::new(),
            images: Vec::new(),
            hyperlinks: Vec::new(),
            headers: Vec::new(),
            footers: Vec::new(),
            next_table_id: 1,
            next_image_id: 1,
            metadata: HwpxMetadata::default(),
        }
    }

    pub fn add_paragraph(&mut self, text: &str) -> Result<()> {
        let paragraph = Paragraph {
            text: Some(ParaText {
                content: text.to_string(),
            }),
            ..Default::default()
        };

        self.push_paragraph(paragraph);
        Ok(())
    }

    pub fn add_styled_paragraph(&mut self, text: &str, style: HwpxTextStyle) -> Result<()> {
        let char_shape = style.to_char_shape();
        let char_shape_id = self.add_char_shape(char_shape);

        let paragraph = Paragraph {
            text: Some(ParaText {
                content: text.to_string(),
            }),
            char_shapes: Some(ParaCharShape {
                char_positions: vec![CharPositionShape {
                    position: 0,
                    char_shape_id,
                }],
            }),
            ..Default::default()
        };

        self.push_paragraph(paragraph);
        Ok(())
    }

    pub fn add_mixed_styled_paragraph(&mut self, runs: Vec<StyledText>) -> Result<()> {
        let mut full_text = String::new();
        let mut char_positions = Vec::new();
        let mut position: u32 = 0;

        for run in runs {
            let char_shape = run.style.to_char_shape();
            let char_shape_id = self.add_char_shape(char_shape);

            char_positions.push(CharPositionShape {
                position,
                char_shape_id,
            });

            position += run.text.chars().count() as u32;
            full_text.push_str(&run.text);
        }

        let paragraph = Paragraph {
            text: Some(ParaText { content: full_text }),
            char_shapes: Some(ParaCharShape { char_positions }),
            ..Default::default()
        };

        self.push_paragraph(paragraph);
        Ok(())
    }

    pub fn add_table(&mut self, table: HwpxTable) -> Result<()> {
        let para_idx = self.current_paragraph_count();
        self.tables.push((para_idx, table));

        let paragraph = Paragraph {
            text: Some(ParaText {
                content: String::new(),
            }),
            ..Default::default()
        };
        self.push_paragraph(paragraph);
        Ok(())
    }

    pub fn add_image(&mut self, image: HwpxImage) -> Result<()> {
        let para_idx = self.current_paragraph_count();
        self.images.push((para_idx, image));

        let paragraph = Paragraph {
            text: Some(ParaText {
                content: String::new(),
            }),
            ..Default::default()
        };
        self.push_paragraph(paragraph);
        Ok(())
    }

    pub fn add_image_from_file<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<()> {
        let data = std::fs::read(path).map_err(HwpError::Io)?;
        let image = HwpxImage::from_bytes(data)
            .ok_or_else(|| HwpError::ParseError("Unsupported image format".to_string()))?;
        self.add_image(image)
    }

    pub fn add_paragraph_with_hyperlinks(
        &mut self,
        text: &str,
        links: Vec<HwpxHyperlink>,
    ) -> Result<()> {
        let para_idx = self.current_paragraph_count();
        self.hyperlinks.push((para_idx, links));

        let paragraph = Paragraph {
            text: Some(ParaText {
                content: text.to_string(),
            }),
            ..Default::default()
        };
        self.push_paragraph(paragraph);
        Ok(())
    }

    pub fn add_hyperlink(&mut self, display_text: &str, url: &str) -> Result<()> {
        self.add_paragraph_with_hyperlinks(
            display_text,
            vec![HwpxHyperlink::new(display_text, url)],
        )
    }

    pub fn add_header(&mut self, text: &str) {
        self.headers.push(HwpxHeader::new(text));
    }

    pub fn add_header_config(&mut self, header: HwpxHeader) {
        self.headers.push(header);
    }

    pub fn add_footer(&mut self, text: &str) {
        self.footers.push(HwpxFooter::new(text));
    }

    pub fn add_footer_with_page_number(&mut self, prefix: &str) {
        self.footers
            .push(HwpxFooter::new(prefix).with_page_number());
    }

    pub fn add_footer_config(&mut self, footer: HwpxFooter) {
        self.footers.push(footer);
    }

    fn current_paragraph_count(&self) -> usize {
        self.document
            .body_texts
            .iter()
            .flat_map(|b| &b.sections)
            .flat_map(|s| &s.paragraphs)
            .count()
    }

    fn push_paragraph(&mut self, paragraph: Paragraph) {
        if self.document.body_texts.is_empty() {
            self.document.body_texts.push(BodyText {
                sections: vec![Section {
                    paragraphs: vec![paragraph],
                    section_def: None,
                    page_def: None,
                }],
            });
        } else if let Some(body) = self.document.body_texts.first_mut() {
            if let Some(section) = body.sections.first_mut() {
                section.paragraphs.push(paragraph);
            }
        }
    }

    fn add_char_shape(&mut self, char_shape: CharShape) -> u16 {
        let id = self.document.doc_info.char_shapes.len() as u16;
        self.document.doc_info.char_shapes.push(char_shape);
        id
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buffer = Cursor::new(Vec::new());
        self.write_to(&mut buffer)?;
        Ok(buffer.into_inner())
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = File::create(path).map_err(HwpError::Io)?;
        self.write_to(file)
    }

    fn write_to<W: Write + std::io::Seek>(&self, writer: W) -> Result<()> {
        let mut zip = ZipWriter::new(writer);
        let stored =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // mimetype must be first and uncompressed (per ODF spec)
        zip.start_file("mimetype", stored)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(b"application/hwp+zip")
            .map_err(HwpError::Io)?;

        // version.xml
        zip.start_file("version.xml", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(self.generate_version_xml().as_bytes())
            .map_err(HwpError::Io)?;

        // Contents directory
        zip.add_directory("Contents", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;

        // Contents/header.xml
        zip.start_file("Contents/header.xml", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(self.generate_header_xml().as_bytes())
            .map_err(HwpError::Io)?;

        // Contents/section0.xml (and more if multiple sections)
        for (idx, section_xml) in self.generate_section_xmls().iter().enumerate() {
            let filename = format!("Contents/section{}.xml", idx);
            zip.start_file(&filename, deflated)
                .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
            zip.write_all(section_xml.as_bytes())
                .map_err(HwpError::Io)?;
        }

        // Preview directory
        zip.add_directory("Preview", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;

        // Preview/PrvText.txt
        zip.start_file("Preview/PrvText.txt", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(self.generate_preview_text().as_bytes())
            .map_err(HwpError::Io)?;

        // Scripts directory
        zip.add_directory("Scripts", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;

        // Scripts/headerScripts (empty but required)
        zip.start_file("Scripts/headerScripts", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(&self.generate_header_scripts())
            .map_err(HwpError::Io)?;

        // Scripts/sourceScripts (empty but required)
        zip.start_file("Scripts/sourceScripts", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(&self.generate_source_scripts())
            .map_err(HwpError::Io)?;

        // settings.xml
        zip.start_file("settings.xml", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(self.generate_settings_xml().as_bytes())
            .map_err(HwpError::Io)?;

        // META-INF directory
        zip.add_directory("META-INF", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;

        // META-INF/container.xml
        zip.start_file("META-INF/container.xml", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(self.generate_container_xml().as_bytes())
            .map_err(HwpError::Io)?;

        // META-INF/manifest.xml
        zip.start_file("META-INF/manifest.xml", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(self.generate_manifest_xml().as_bytes())
            .map_err(HwpError::Io)?;

        // META-INF/container.rdf
        zip.start_file("META-INF/container.rdf", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(self.generate_container_rdf().as_bytes())
            .map_err(HwpError::Io)?;

        // Contents/content.hpf (must be after sections are known)
        zip.start_file("Contents/content.hpf", deflated)
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
        zip.write_all(self.generate_content_hpf().as_bytes())
            .map_err(HwpError::Io)?;

        if !self.images.is_empty() {
            zip.add_directory("BinData", deflated)
                .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;

            for (idx, (_, image)) in self.images.iter().enumerate() {
                let filename = format!("BinData/image{}.{}", idx + 1, image.format.extension());
                zip.start_file(&filename, stored)
                    .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;
                zip.write_all(&image.data).map_err(HwpError::Io)?;
            }
        }

        zip.finish()
            .map_err(|e| HwpError::Io(std::io::Error::other(e)))?;

        Ok(())
    }

    fn generate_version_xml(&self) -> String {
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>"#,
            r#"<hv:HCFVersion xmlns:hv="http://www.hancom.co.kr/hwpml/2011/version" "#,
            r#"tagetApplication="WORDPROCESSOR" major="5" minor="1" micro="1" "#,
            r#"buildNumber="0" os="1" xmlVersion="1.5" application="Hancom Office Hangul" "#,
            r#"appVersion="12, 0, 0, 0"/>"#
        )
        .to_string()
    }

    fn generate_settings_xml(&self) -> String {
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>"#,
            r#"<ha:HWPApplicationSetting xmlns:ha="http://www.hancom.co.kr/hwpml/2011/app" "#,
            r#"xmlns:config="urn:oasis:names:tc:opendocument:xmlns:config:1.0">"#,
            r#"<ha:CaretPosition listIDRef="0" paraIDRef="0" pos="0"/>"#,
            r#"</ha:HWPApplicationSetting>"#
        )
        .to_string()
    }

    fn generate_container_xml(&self) -> String {
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>"#,
            r#"<ocf:container xmlns:ocf="urn:oasis:names:tc:opendocument:xmlns:container" "#,
            r#"xmlns:hpf="http://www.hancom.co.kr/schema/2011/hpf">"#,
            r#"<ocf:rootfiles>"#,
            r#"<ocf:rootfile full-path="Contents/content.hpf" media-type="application/hwpml-package+xml"/>"#,
            r#"<ocf:rootfile full-path="Preview/PrvText.txt" media-type="text/plain"/>"#,
            r#"<ocf:rootfile full-path="META-INF/container.rdf" media-type="application/rdf+xml"/>"#,
            r#"</ocf:rootfiles></ocf:container>"#
        )
        .to_string()
    }

    fn generate_manifest_xml(&self) -> String {
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>"#,
            r#"<odf:manifest xmlns:odf="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"/>"#
        )
        .to_string()
    }

    fn generate_container_rdf(&self) -> String {
        let section_count = self.get_section_count();
        let mut rdf = String::from(concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>"#,
            r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">"#,
            r#"<rdf:Description rdf:about="">"#,
            r#"<ns0:hasPart xmlns:ns0="http://www.hancom.co.kr/hwpml/2016/meta/pkg#" rdf:resource="Contents/header.xml"/>"#,
            r#"</rdf:Description>"#,
            r#"<rdf:Description rdf:about="Contents/header.xml">"#,
            r#"<rdf:type rdf:resource="http://www.hancom.co.kr/hwpml/2016/meta/pkg#HeaderFile"/>"#,
            r#"</rdf:Description>"#
        ));

        for idx in 0..section_count {
            rdf.push_str(&format!(
                concat!(
                    r#"<rdf:Description rdf:about="">"#,
                    r#"<ns0:hasPart xmlns:ns0="http://www.hancom.co.kr/hwpml/2016/meta/pkg#" rdf:resource="Contents/section{}.xml"/>"#,
                    r#"</rdf:Description>"#,
                    r#"<rdf:Description rdf:about="Contents/section{}.xml">"#,
                    r#"<rdf:type rdf:resource="http://www.hancom.co.kr/hwpml/2016/meta/pkg#SectionFile"/>"#,
                    r#"</rdf:Description>"#
                ),
                idx, idx
            ));
        }

        rdf.push_str(concat!(
            r#"<rdf:Description rdf:about="">"#,
            r#"<rdf:type rdf:resource="http://www.hancom.co.kr/hwpml/2016/meta/pkg#Document"/>"#,
            r#"</rdf:Description></rdf:RDF>"#
        ));
        rdf
    }

    fn generate_content_hpf(&self) -> String {
        let section_count = self.get_section_count();

        let mut sections_manifest = String::new();
        let mut sections_spine = String::new();
        for idx in 0..section_count {
            sections_manifest.push_str(&format!(
                r#"<opf:item id="section{}" href="Contents/section{}.xml" media-type="application/xml"/>"#,
                idx, idx
            ));
            sections_spine.push_str(&format!(
                r#"<opf:itemref idref="section{}" linear="yes"/>"#,
                idx
            ));
        }

        let mut images_manifest = String::new();
        for (idx, (_, image)) in self.images.iter().enumerate() {
            let item_id = format!("image{}", idx + 1);
            let href = format!("BinData/image{}.{}", idx + 1, image.format.extension());
            let media_type = match image.format {
                HwpxImageFormat::Png => "image/png",
                HwpxImageFormat::Jpeg => "image/jpg",
                HwpxImageFormat::Gif => "image/gif",
                HwpxImageFormat::Bmp => "image/bmp",
            };
            images_manifest.push_str(&format!(
                r#"<opf:item id="{}" href="{}" media-type="{}" isEmbeded="1"/>"#,
                item_id, href, media_type
            ));
        }

        let title = xml_escape(&self.metadata.title);
        let creator = xml_escape(&self.metadata.creator);
        let created_date = xml_escape(&self.metadata.created_date);

        format!(
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>"#,
                r#"<opf:package {} version="" unique-identifier="" id="">"#,
                r#"<opf:metadata>"#,
                r#"<opf:title>{}</opf:title>"#,
                r#"<opf:language>ko</opf:language>"#,
                r#"<opf:meta name="creator" content="text">{}</opf:meta>"#,
                r#"<opf:meta name="subject" content="text"/>"#,
                r#"<opf:meta name="description" content="text"/>"#,
                r#"<opf:meta name="lastsaveby" content="text">{}</opf:meta>"#,
                r#"<opf:meta name="CreatedDate" content="text">{}</opf:meta>"#,
                r#"<opf:meta name="ModifiedDate" content="text">{}</opf:meta>"#,
                r#"<opf:meta name="date" content="text">{}</opf:meta>"#,
                r#"<opf:meta name="keyword" content="text"/>"#,
                r#"</opf:metadata>"#,
                r#"<opf:manifest>"#,
                r#"<opf:item id="header" href="Contents/header.xml" media-type="application/xml"/>"#,
                r#"{}"#,
                r#"{}"#,
                r#"<opf:item id="headersc" href="Scripts/headerScripts" media-type="application/x-javascript ;charset=utf-16"/>"#,
                r#"<opf:item id="sourcesc" href="Scripts/sourceScripts" media-type="application/x-javascript ;charset=utf-16"/>"#,
                r#"<opf:item id="settings" href="settings.xml" media-type="application/xml"/>"#,
                r#"</opf:manifest>"#,
                r#"<opf:spine>"#,
                r#"<opf:itemref idref="header" linear="yes"/>"#,
                r#"{}"#,
                r#"<opf:itemref idref="headersc" linear="yes"/>"#,
                r#"<opf:itemref idref="sourcesc" linear="yes"/>"#,
                r#"</opf:spine></opf:package>"#
            ),
            HWPX_NAMESPACES,
            title,          // opf:title
            creator,        // creator
            creator,        // lastsaveby
            created_date,   // CreatedDate
            created_date,   // ModifiedDate
            created_date,   // date
            sections_manifest,
            images_manifest,
            sections_spine
        )
    }

    fn generate_header_xml(&self) -> String {
        let mut xml = String::new();
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>"#);
        xml.push_str("<hh:head ");
        xml.push_str(HWPX_NAMESPACES);
        xml.push_str(r#" version="1.5" secCnt="1">"#);
        xml.push_str(
            r#"<hh:beginNum page="1" footnote="1" endnote="1" pic="1" tbl="1" equation="1"/>"#,
        );
        xml.push_str("<hh:refList>");

        // fontfaces
        xml.push_str(r#"<hh:fontfaces itemCnt="7">"#);
        xml.push_str(r#"<hh:fontface lang="HANGUL" fontCnt="1"><hh:font id="0" face="맑은 고딕" type="TTF" isEmbedded="0"><hh:typeInfo weight="26" proportion="26" contrast="26" strokeVariation="26" armStyle="26" letterform="26" midline="26" xHeight="26"/></hh:font></hh:fontface>"#);
        xml.push_str(r#"<hh:fontface lang="LATIN" fontCnt="1"><hh:font id="0" face="맑은 고딕" type="TTF" isEmbedded="0"><hh:typeInfo familyType="FCAT_UNKNOWN" weight="0" proportion="0" contrast="0" strokeVariation="0" armStyle="0" letterform="0" midline="252" xHeight="255"/></hh:font></hh:fontface>"#);
        xml.push_str(r#"<hh:fontface lang="HANJA" fontCnt="1"><hh:font id="0" face="맑은 고딕" type="TTF" isEmbedded="0"><hh:typeInfo familyType="FCAT_UNKNOWN" weight="0" proportion="0" contrast="0" strokeVariation="0" armStyle="0" letterform="0" midline="252" xHeight="255"/></hh:font></hh:fontface>"#);
        xml.push_str(r#"<hh:fontface lang="JAPANESE" fontCnt="1"><hh:font id="0" face="맑은 고딕" type="TTF" isEmbedded="0"><hh:typeInfo familyType="FCAT_UNKNOWN" weight="0" proportion="0" contrast="0" strokeVariation="0" armStyle="0" letterform="0" midline="252" xHeight="255"/></hh:font></hh:fontface>"#);
        xml.push_str(r#"<hh:fontface lang="OTHER" fontCnt="1"><hh:font id="0" face="맑은 고딕" type="TTF" isEmbedded="0"><hh:typeInfo familyType="FCAT_UNKNOWN" weight="0" proportion="0" contrast="0" strokeVariation="0" armStyle="0" letterform="0" midline="252" xHeight="255"/></hh:font></hh:fontface>"#);
        xml.push_str(r#"<hh:fontface lang="SYMBOL" fontCnt="1"><hh:font id="0" face="맑은 고딕" type="TTF" isEmbedded="0"><hh:typeInfo familyType="FCAT_UNKNOWN" weight="0" proportion="0" contrast="0" strokeVariation="0" armStyle="0" letterform="0" midline="252" xHeight="255"/></hh:font></hh:fontface>"#);
        xml.push_str(r#"<hh:fontface lang="USER" fontCnt="1"><hh:font id="0" face="맑은 고딕" type="TTF" isEmbedded="0"><hh:typeInfo familyType="FCAT_UNKNOWN" weight="0" proportion="0" contrast="0" strokeVariation="0" armStyle="0" letterform="0" midline="252" xHeight="255"/></hh:font></hh:fontface>"#);
        xml.push_str("</hh:fontfaces>");

        xml.push_str(r#"<hh:borderFills itemCnt="3">"#);
        xml.push_str(r#"<hh:borderFill id="1" threeD="0" shadow="0" centerLine="NONE" breakCellSeparateLine="0">"#);
        xml.push_str(r#"<hh:slash type="NONE" Crooked="0" isCounter="0"/><hh:backSlash type="NONE" Crooked="0" isCounter="0"/>"#);
        xml.push_str("<hh:leftBorder type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/><hh:rightBorder type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/>");
        xml.push_str("<hh:topBorder type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/><hh:bottomBorder type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/>");
        xml.push_str(
            "<hh:diagonal type=\"SOLID\" width=\"0.1 mm\" color=\"#000000\"/></hh:borderFill>",
        );
        xml.push_str(r#"<hh:borderFill id="2" threeD="0" shadow="0" centerLine="NONE" breakCellSeparateLine="0">"#);
        xml.push_str(r#"<hh:slash type="NONE" Crooked="0" isCounter="0"/><hh:backSlash type="NONE" Crooked="0" isCounter="0"/>"#);
        xml.push_str("<hh:leftBorder type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/><hh:rightBorder type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/>");
        xml.push_str("<hh:topBorder type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/><hh:bottomBorder type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/>");
        xml.push_str("<hh:diagonal type=\"SOLID\" width=\"0.1 mm\" color=\"#000000\"/>");
        xml.push_str("<hc:fillBrush><hc:winBrush faceColor=\"none\" hatchColor=\"#999999\" alpha=\"0\"/></hc:fillBrush></hh:borderFill>");
        // id="3": 테이블 셀용 (실선 테두리)
        xml.push_str(r#"<hh:borderFill id="3" threeD="0" shadow="0" centerLine="NONE" breakCellSeparateLine="0">"#);
        xml.push_str(r#"<hh:slash type="NONE" Crooked="0" isCounter="0"/><hh:backSlash type="NONE" Crooked="0" isCounter="0"/>"#);
        xml.push_str("<hh:leftBorder type=\"SOLID\" width=\"0.12 mm\" color=\"#000000\"/><hh:rightBorder type=\"SOLID\" width=\"0.12 mm\" color=\"#000000\"/>");
        xml.push_str("<hh:topBorder type=\"SOLID\" width=\"0.12 mm\" color=\"#000000\"/><hh:bottomBorder type=\"SOLID\" width=\"0.12 mm\" color=\"#000000\"/>");
        xml.push_str("<hh:diagonal type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/></hh:borderFill>");
        xml.push_str("</hh:borderFills>");

        xml.push_str(&self.generate_char_properties());

        // tabProperties
        xml.push_str(r#"<hh:tabProperties itemCnt="1"><hh:tabPr id="0" autoTabLeft="0" autoTabRight="0"/></hh:tabProperties>"#);

        // numberings
        xml.push_str(r#"<hh:numberings itemCnt="1"><hh:numbering id="1" start="0">"#);
        xml.push_str(r#"<hh:paraHead start="1" level="1" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="DIGIT" charPrIDRef="4294967295" checkable="0">^1.</hh:paraHead>"#);
        xml.push_str(r#"<hh:paraHead start="1" level="2" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="HANGUL_SYLLABLE" charPrIDRef="4294967295" checkable="0">^2.</hh:paraHead>"#);
        xml.push_str(r#"<hh:paraHead start="1" level="3" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="DIGIT" charPrIDRef="4294967295" checkable="0">^3)</hh:paraHead>"#);
        xml.push_str(r#"<hh:paraHead start="1" level="4" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="HANGUL_SYLLABLE" charPrIDRef="4294967295" checkable="0">^4)</hh:paraHead>"#);
        xml.push_str(r#"<hh:paraHead start="1" level="5" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="DIGIT" charPrIDRef="4294967295" checkable="0">(^5)</hh:paraHead>"#);
        xml.push_str(r#"<hh:paraHead start="1" level="6" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="HANGUL_SYLLABLE" charPrIDRef="4294967295" checkable="0">(^6)</hh:paraHead>"#);
        xml.push_str(r#"<hh:paraHead start="1" level="7" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="CIRCLED_DIGIT" charPrIDRef="4294967295" checkable="1">^7</hh:paraHead>"#);
        xml.push_str(r#"<hh:paraHead start="1" level="8" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="CIRCLED_HANGUL_SYLLABLE" charPrIDRef="4294967295" checkable="1">^8</hh:paraHead>"#);
        xml.push_str(r#"<hh:paraHead start="1" level="9" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="HANGUL_JAMO" charPrIDRef="4294967295" checkable="0"/>"#);
        xml.push_str(r#"<hh:paraHead start="1" level="10" align="LEFT" useInstWidth="1" autoIndent="1" widthAdjust="0" textOffsetType="PERCENT" textOffset="50" numFormat="ROMAN_SMALL" charPrIDRef="4294967295" checkable="1"/>"#);
        xml.push_str("</hh:numbering></hh:numberings>");

        // paraProperties
        xml.push_str(r#"<hh:paraProperties itemCnt="1">"#);
        xml.push_str(r#"<hh:paraPr id="0" tabPrIDRef="0" condense="0" fontLineHeight="0" snapToGrid="1" suppressLineNumbers="0" checked="0">"#);
        xml.push_str(r#"<hh:align horizontal="JUSTIFY" vertical="BASELINE"/>"#);
        xml.push_str(r#"<hh:heading type="NONE" idRef="0" level="0"/>"#);
        xml.push_str(r#"<hh:breakSetting breakLatinWord="KEEP_WORD" breakNonLatinWord="KEEP_WORD" widowOrphan="0" keepWithNext="0" keepLines="0" pageBreakBefore="0" lineWrap="BREAK"/>"#);
        xml.push_str(r#"<hh:autoSpacing eAsianEng="0" eAsianNum="0"/>"#);
        xml.push_str(r#"<hp:switch><hp:case hp:required-namespace="http://www.hancom.co.kr/hwpml/2016/HwpUnitChar">"#);
        xml.push_str(r#"<hh:margin><hc:intent value="0" unit="HWPUNIT"/><hc:left value="0" unit="HWPUNIT"/><hc:right value="0" unit="HWPUNIT"/><hc:prev value="0" unit="HWPUNIT"/><hc:next value="0" unit="HWPUNIT"/></hh:margin>"#);
        xml.push_str(r#"<hh:lineSpacing type="PERCENT" value="160" unit="HWPUNIT"/></hp:case>"#);
        xml.push_str(r#"<hp:default><hh:margin><hc:intent value="0" unit="HWPUNIT"/><hc:left value="0" unit="HWPUNIT"/><hc:right value="0" unit="HWPUNIT"/><hc:prev value="0" unit="HWPUNIT"/><hc:next value="0" unit="HWPUNIT"/></hh:margin>"#);
        xml.push_str(r#"<hh:lineSpacing type="PERCENT" value="160" unit="HWPUNIT"/></hp:default></hp:switch>"#);
        xml.push_str(r#"<hh:border borderFillIDRef="2" offsetLeft="0" offsetRight="0" offsetTop="0" offsetBottom="0" connect="0" ignoreMargin="0"/>"#);
        xml.push_str("</hh:paraPr></hh:paraProperties>");

        // styles
        xml.push_str(r#"<hh:styles itemCnt="1">"#);
        xml.push_str(r#"<hh:style id="0" type="PARA" name="바탕글" engName="Normal" paraPrIDRef="0" charPrIDRef="0" nextStyleIDRef="0" langID="1042" lockForm="0"/>"#);
        xml.push_str("</hh:styles>");

        xml.push_str(&self.generate_bin_data_items());

        xml.push_str("</hh:refList>");

        // compatibleDocument
        xml.push_str(r#"<hh:compatibleDocument targetProgram="HWP201X"><hh:layoutCompatibility/></hh:compatibleDocument>"#);

        // docOption
        xml.push_str(r#"<hh:docOption><hh:linkinfo path="" pageInherit="0" footnoteInherit="0"/></hh:docOption>"#);

        // trackchangeConfig
        xml.push_str(r#"<hh:trackchageConfig flags="56">"#);
        xml.push_str(r#"<config:config-item-set name="TrackChangePasswordInfo">"#);
        xml.push_str(
            r#"<config:config-item name="algorithm-name" type="string">SHA1</config:config-item>"#,
        );
        xml.push_str("</config:config-item-set></hh:trackchageConfig>");
        xml.push_str("</hh:head>");

        xml
    }

    fn generate_bin_data_items(&self) -> String {
        if self.images.is_empty() {
            return String::new();
        }

        let mut xml = format!(r#"<hh:binDataItems itemCnt="{}">"#, self.images.len());
        for (idx, (_, image)) in self.images.iter().enumerate() {
            let item_id = format!("image{}", idx + 1);
            let src = format!("BinData/image{}.{}", idx + 1, image.format.extension());
            let format = image.format.extension().to_uppercase();
            xml.push_str(&format!(
                r#"<hh:binDataItem id="{}" src="{}" format="{}" isEmbeded="1"/>"#,
                item_id, src, format
            ));
        }
        xml.push_str("</hh:binDataItems>");
        xml
    }

    fn generate_char_properties(&self) -> String {
        let char_shapes = &self.document.doc_info.char_shapes;
        let count = char_shapes.len().max(1);

        let mut xml = format!(r#"<hh:charProperties itemCnt="{}">"#, count);

        if char_shapes.is_empty() {
            xml.push_str(&self.format_char_pr(0, &CharShape::new_default()));
        } else {
            for (id, cs) in char_shapes.iter().enumerate() {
                xml.push_str(&self.format_char_pr(id as u32, cs));
            }
        }

        xml.push_str("</hh:charProperties>");
        xml
    }

    fn format_char_pr(&self, id: u32, cs: &CharShape) -> String {
        let height = cs.base_size;
        let text_color = format!("#{:06X}", cs.text_color & 0xFFFFFF);
        let underline_color = format!("#{:06X}", cs.underline_color & 0xFFFFFF);
        let shadow_color = format!("#{:06X}", cs.shadow_color & 0xFFFFFF);

        let bold_attr = if cs.is_bold() { r#" bold="1""# } else { "" };
        let italic_attr = if cs.is_italic() { r#" italic="1""# } else { "" };
        let underline_type = if cs.is_underline() { "BOTTOM" } else { "NONE" };
        let strikeout_shape = if cs.is_strikethrough() {
            "CONTINUOUS"
        } else {
            "NONE"
        };

        format!(
            concat!(
                r#"<hh:charPr id="{}" height="{}"{}{} textColor="{}" shadeColor="none" "#,
                r#"useFontSpace="0" useKerning="0" symMark="NONE" borderFillIDRef="2">"#,
                r#"<hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>"#,
                r#"<hh:ratio hangul="100" latin="100" hanja="100" japanese="100" other="100" symbol="100" user="100"/>"#,
                r#"<hh:spacing hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>"#,
                r#"<hh:relSz hangul="100" latin="100" hanja="100" japanese="100" other="100" symbol="100" user="100"/>"#,
                r#"<hh:offset hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>"#,
                r#"<hh:underline type="{}" shape="SOLID" color="{}"/>"#,
                r#"<hh:strikeout shape="{}" color="{}"/>"#,
                r#"<hh:outline type="NONE"/>"#,
                r#"<hh:shadow type="NONE" color="{}" offsetX="10" offsetY="10"/>"#,
                r#"</hh:charPr>"#
            ),
            id,
            height,
            bold_attr,
            italic_attr,
            text_color,
            underline_type,
            underline_color,
            strikeout_shape,
            text_color,
            shadow_color
        )
    }

    fn generate_header_scripts(&self) -> Vec<u8> {
        vec![0xFF, 0xFE]
    }

    fn generate_source_scripts(&self) -> Vec<u8> {
        // UTF-16LE BOM + empty content (as in real Hanword files)
        vec![0xFF, 0xFE]
    }

    fn generate_preview_text(&self) -> String {
        // Extract text from all paragraphs for preview
        let mut text = String::new();
        for body in &self.document.body_texts {
            for section in &body.sections {
                for para in &section.paragraphs {
                    if let Some(para_text) = &para.text {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&para_text.content);
                    }
                }
            }
        }
        if text.is_empty() {
            text = String::from(" ");
        }
        text
    }

    fn generate_section_xmls(&self) -> Vec<String> {
        if self.document.body_texts.is_empty() {
            return vec![self.generate_empty_section()];
        }

        self.document
            .body_texts
            .iter()
            .flat_map(|body| &body.sections)
            .map(|section| self.generate_section_xml(section))
            .collect()
    }

    fn generate_empty_section(&self) -> String {
        self.generate_section_xml_with_paragraphs(&[])
    }

    fn generate_section_xml(&self, section: &crate::model::Section) -> String {
        let paragraphs: Vec<_> = section.paragraphs.iter().collect();
        self.generate_section_xml_with_paragraphs(&paragraphs)
    }

    fn generate_section_xml_with_paragraphs(
        &self,
        paragraphs: &[&crate::model::Paragraph],
    ) -> String {
        let mut sec_pr = String::new();
        sec_pr.push_str(
            r#"<hp:secPr id="" textDirection="HORIZONTAL" spaceColumns="1134" tabStop="8000" tabStopVal="4000" tabStopUnit="HWPUNIT" outlineShapeIDRef="1" memoShapeIDRef="0" textVerticalWidthHead="0" masterPageCnt="0">"#
        );
        sec_pr.push_str(r#"<hp:grid lineGrid="0" charGrid="0" wonggojiFormat="0"/>"#);
        sec_pr.push_str(
            r#"<hp:startNum pageStartsOn="BOTH" page="0" pic="0" tbl="0" equation="0"/>"#,
        );
        sec_pr.push_str(r#"<hp:visibility hideFirstHeader="0" hideFirstFooter="0" hideFirstMasterPage="0" border="SHOW_ALL" fill="SHOW_ALL" hideFirstPageNum="0" hideFirstEmptyLine="0" showLineNumber="0"/>"#);
        sec_pr.push_str(
            r#"<hp:lineNumberShape restartType="0" countBy="0" distance="0" startNumber="0"/>"#,
        );
        sec_pr.push_str(
            r#"<hp:pagePr landscape="WIDELY" width="59528" height="84186" gutterType="LEFT_ONLY">"#,
        );
        sec_pr.push_str(r#"<hp:margin header="4252" footer="4252" gutter="0" left="8504" right="8504" top="5668" bottom="4252"/></hp:pagePr>"#);
        sec_pr.push_str(r#"<hp:footNotePr><hp:autoNumFormat type="DIGIT" userChar="" prefixChar="" suffixChar=")" supscript="0"/>"#);
        sec_pr.push_str(
            "<hp:noteLine length=\"-1\" type=\"SOLID\" width=\"0.12 mm\" color=\"#000000\"/>",
        );
        sec_pr.push_str(r#"<hp:noteSpacing betweenNotes="283" belowLine="567" aboveLine="850"/>"#);
        sec_pr.push_str(r#"<hp:numbering type="CONTINUOUS" newNum="1"/><hp:placement place="EACH_COLUMN" beneathText="0"/></hp:footNotePr>"#);
        sec_pr.push_str(r#"<hp:endNotePr><hp:autoNumFormat type="DIGIT" userChar="" prefixChar="" suffixChar=")" supscript="0"/>"#);
        sec_pr.push_str(
            "<hp:noteLine length=\"14692344\" type=\"SOLID\" width=\"0.12 mm\" color=\"#000000\"/>",
        );
        sec_pr.push_str(r#"<hp:noteSpacing betweenNotes="0" belowLine="567" aboveLine="850"/>"#);
        sec_pr.push_str(r#"<hp:numbering type="CONTINUOUS" newNum="1"/><hp:placement place="END_OF_DOCUMENT" beneathText="0"/></hp:endNotePr>"#);
        sec_pr.push_str(r#"<hp:pageBorderFill type="BOTH" borderFillIDRef="1" textBorder="PAPER" headerInside="0" footerInside="0" fillArea="PAPER">"#);
        sec_pr.push_str(
            r#"<hp:offset left="1417" right="1417" top="1417" bottom="1417"/></hp:pageBorderFill>"#,
        );
        sec_pr.push_str(r#"<hp:pageBorderFill type="EVEN" borderFillIDRef="1" textBorder="PAPER" headerInside="0" footerInside="0" fillArea="PAPER">"#);
        sec_pr.push_str(
            r#"<hp:offset left="1417" right="1417" top="1417" bottom="1417"/></hp:pageBorderFill>"#,
        );
        sec_pr.push_str(r#"<hp:pageBorderFill type="ODD" borderFillIDRef="1" textBorder="PAPER" headerInside="0" footerInside="0" fillArea="PAPER">"#);
        sec_pr.push_str(
            r#"<hp:offset left="1417" right="1417" top="1417" bottom="1417"/></hp:pageBorderFill>"#,
        );
        sec_pr.push_str("</hp:secPr>");
        sec_pr.push_str(r#"<hp:ctrl><hp:colPr id="" type="NEWSPAPER" layout="LEFT" colCount="1" sameSz="1" sameGap="0"/></hp:ctrl>"#);

        let mut xml = format!(
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>"#,
                r#"<hs:sec {}>"#
            ),
            HWPX_NAMESPACES
        );

        let has_headers = !self.headers.is_empty();
        let has_footers = !self.footers.is_empty();

        // First paragraph with section properties
        if paragraphs.is_empty() {
            xml.push_str(concat!(
                r#"<hp:p id="0" paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0">"#,
                r#"<hp:run charPrIDRef="0">"#
            ));
            xml.push_str(&sec_pr);
            xml.push_str(r#"</hp:run>"#);
            if has_headers {
                xml.push_str(r#"<hp:run charPrIDRef="0">"#);
                xml.push_str(&self.generate_header_ctrl_xml());
                xml.push_str(r#"<hp:t/></hp:run>"#);
            }
            if has_footers {
                xml.push_str(r#"<hp:run charPrIDRef="0">"#);
                xml.push_str(&self.generate_footer_ctrl_xml());
                xml.push_str(r#"<hp:t/></hp:run>"#);
            }
            xml.push_str(r#"<hp:run charPrIDRef="0"><hp:t></hp:t></hp:run></hp:p>"#);
        } else {
            for (idx, para) in paragraphs.iter().enumerate() {
                let para_pr_id = para.para_shape_id;

                xml.push_str(&format!(
                    r#"<hp:p id="{}" paraPrIDRef="{}" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0">"#,
                    idx, para_pr_id
                ));

                if idx == 0 {
                    xml.push_str(r#"<hp:run charPrIDRef="0">"#);
                    xml.push_str(&sec_pr);
                    xml.push_str(r#"</hp:run>"#);
                    if has_headers {
                        xml.push_str(r#"<hp:run charPrIDRef="0">"#);
                        xml.push_str(&self.generate_header_ctrl_xml());
                        xml.push_str(r#"<hp:t/></hp:run>"#);
                    }
                    if has_footers {
                        xml.push_str(r#"<hp:run charPrIDRef="0">"#);
                        xml.push_str(&self.generate_footer_ctrl_xml());
                        xml.push_str(r#"<hp:t/></hp:run>"#);
                    }
                }

                let text = para.text.as_ref().map(|t| t.content.as_str()).unwrap_or("");

                if let Some(char_shapes) = &para.char_shapes {
                    let mut last_pos = 0;
                    for (i, pos_shape) in char_shapes.char_positions.iter().enumerate() {
                        let start = pos_shape.position as usize;
                        let end = char_shapes
                            .char_positions
                            .get(i + 1)
                            .map(|p| p.position as usize)
                            .unwrap_or(text.chars().count());

                        if start > last_pos {
                            let segment: String =
                                text.chars().skip(last_pos).take(start - last_pos).collect();
                            if !segment.is_empty() {
                                xml.push_str(&format!(
                                    r#"<hp:run charPrIDRef="0"><hp:t>{}</hp:t></hp:run>"#,
                                    escape_xml(&segment)
                                ));
                            }
                        }

                        let segment: String = text.chars().skip(start).take(end - start).collect();
                        if !segment.is_empty() {
                            xml.push_str(&format!(
                                r#"<hp:run charPrIDRef="{}"><hp:t>{}</hp:t></hp:run>"#,
                                pos_shape.char_shape_id,
                                escape_xml(&segment)
                            ));
                        }
                        last_pos = end;
                    }

                    if last_pos < text.chars().count() {
                        let remaining: String = text.chars().skip(last_pos).collect();
                        xml.push_str(&format!(
                            r#"<hp:run charPrIDRef="0"><hp:t>{}</hp:t></hp:run>"#,
                            escape_xml(&remaining)
                        ));
                    }
                } else if let Some(table) = self.get_table_for_paragraph(idx) {
                    xml.push_str(r#"<hp:run charPrIDRef="0">"#);
                    xml.push_str(&self.format_table(table));
                    xml.push_str("<hp:t/>");
                    xml.push_str("</hp:run>");
                } else if let Some((img_idx, image)) = self.get_image_for_paragraph(idx) {
                    xml.push_str(r#"<hp:run charPrIDRef="0">"#);
                    xml.push_str(&self.format_picture(img_idx, image));
                    xml.push_str("<hp:t/>");
                    xml.push_str("</hp:run>");
                } else if let Some(links) = self.get_hyperlinks_for_paragraph(idx) {
                    xml.push_str(&self.format_hyperlinks(text, links));
                } else {
                    xml.push_str(&format!(
                        r#"<hp:run charPrIDRef="0"><hp:t>{}</hp:t></hp:run>"#,
                        escape_xml(text)
                    ));
                }

                xml.push_str("</hp:p>");
            }
        }

        xml.push_str("</hs:sec>");
        xml
    }

    fn get_table_for_paragraph(&self, para_idx: usize) -> Option<&HwpxTable> {
        self.tables
            .iter()
            .find(|(idx, _)| *idx == para_idx)
            .map(|(_, table)| table)
    }

    fn get_image_for_paragraph(&self, para_idx: usize) -> Option<(usize, &HwpxImage)> {
        self.images
            .iter()
            .enumerate()
            .find(|(_, (idx, _))| *idx == para_idx)
            .map(|(img_idx, (_, image))| (img_idx, image))
    }

    fn get_hyperlinks_for_paragraph(&self, para_idx: usize) -> Option<&Vec<HwpxHyperlink>> {
        self.hyperlinks
            .iter()
            .find(|(idx, _)| *idx == para_idx)
            .map(|(_, links)| links)
    }

    fn format_hyperlinks(&self, text: &str, links: &[HwpxHyperlink]) -> String {
        let mut xml = String::new();
        let mut last_end = 0usize;

        for link in links {
            if let Some(start) = text.find(&link.text) {
                if start > last_end {
                    let prefix: String =
                        text.chars().skip(last_end).take(start - last_end).collect();
                    xml.push_str(&format!(
                        r#"<hp:run charPrIDRef="0"><hp:t>{}</hp:t></hp:run>"#,
                        escape_xml(&prefix)
                    ));
                }

                xml.push_str(&format!(
                    concat!(
                        r#"<hp:run charPrIDRef="0">"#,
                        r#"<hp:ctrl>"#,
                        r#"<hp:hyperlink url="{}" visited="0" visited_style="0" new_window="0"/>"#,
                        r#"</hp:ctrl>"#,
                        r#"<hp:t>{}</hp:t>"#,
                        r#"</hp:run>"#
                    ),
                    escape_xml(&link.url),
                    escape_xml(&link.text)
                ));

                last_end = start + link.text.chars().count();
            }
        }

        if last_end < text.chars().count() {
            let suffix: String = text.chars().skip(last_end).collect();
            xml.push_str(&format!(
                r#"<hp:run charPrIDRef="0"><hp:t>{}</hp:t></hp:run>"#,
                escape_xml(&suffix)
            ));
        }

        if xml.is_empty() {
            xml.push_str(&format!(
                r#"<hp:run charPrIDRef="0"><hp:t>{}</hp:t></hp:run>"#,
                escape_xml(text)
            ));
        }

        xml
    }

    fn format_picture(&self, img_idx: usize, image: &HwpxImage) -> String {
        let hwp_scale: f64 = 7200.0 / 25.4;
        let content_width: u32 = 42520;

        let org_width = (image.width_mm.unwrap_or(50) as f64 * hwp_scale) as u32;
        let org_height = (image.height_mm.unwrap_or(50) as f64 * hwp_scale) as u32;

        // 본문 너비에 맞게 스케일링
        let (cur_width, cur_height) = if org_width > content_width {
            let scale = content_width as f64 / org_width as f64;
            (content_width, (org_height as f64 * scale) as u32)
        } else {
            (org_width, org_height)
        };

        let item_id = format!("image{}", img_idx + 1);
        let pic_id = self.next_image_id + img_idx as u32;
        let center_x = cur_width / 2;
        let center_y = cur_height / 2;

        let mut xml = String::new();
        xml.push_str(&format!(
            concat!(
                r#"<hp:pic id="{}" zOrder="{}" numberingType="PICTURE" textWrap="TOP_AND_BOTTOM" "#,
                r#"textFlow="BOTH_SIDES" lock="0" dropcapstyle="None" href="" groupLevel="0" "#,
                r#"instid="{}" reverse="0">"#
            ),
            pic_id, img_idx, pic_id
        ));

        xml.push_str(r#"<hp:offset x="0" y="0"/>"#);
        xml.push_str(&format!(
            r#"<hp:orgSz width="{}" height="{}"/>"#,
            org_width, org_height
        ));
        xml.push_str(&format!(
            r#"<hp:curSz width="{}" height="{}"/>"#,
            cur_width, cur_height
        ));
        xml.push_str(r#"<hp:flip horizontal="0" vertical="0"/>"#);
        xml.push_str(&format!(
            r#"<hp:rotationInfo angle="0" centerX="{}" centerY="{}" rotateimage="1"/>"#,
            center_x, center_y
        ));

        // renderingInfo
        let sca_x = if org_width > 0 {
            cur_width as f64 / org_width as f64
        } else {
            1.0
        };
        let sca_y = if org_height > 0 {
            cur_height as f64 / org_height as f64
        } else {
            1.0
        };
        xml.push_str("<hp:renderingInfo>");
        xml.push_str(r#"<hc:transMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/>"#);
        xml.push_str(&format!(
            r#"<hc:scaMatrix e1="{:.6}" e2="0" e3="0" e4="0" e5="{:.6}" e6="0"/>"#,
            sca_x, sca_y
        ));
        xml.push_str(r#"<hc:rotMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/>"#);
        xml.push_str("</hp:renderingInfo>");

        // hc:img (핵심: hp:img 아닌 hc:img 네임스페이스)
        xml.push_str(&format!(
            r#"<hc:img binaryItemIDRef="{}" bright="0" contrast="0" effect="REAL_PIC" alpha="0"/>"#,
            item_id
        ));

        // imgRect
        xml.push_str("<hp:imgRect>");
        xml.push_str(r#"<hc:pt0 x="0" y="0"/>"#);
        xml.push_str(&format!(r#"<hc:pt1 x="{}" y="0"/>"#, org_width));
        xml.push_str(&format!(
            r#"<hc:pt2 x="{}" y="{}"/>"#,
            org_width, org_height
        ));
        xml.push_str(&format!(r#"<hc:pt3 x="0" y="{}"/>"#, org_height));
        xml.push_str("</hp:imgRect>");

        xml.push_str(&format!(
            r#"<hp:imgClip left="0" right="{}" top="0" bottom="{}"/>"#,
            org_width, org_height
        ));
        xml.push_str(r#"<hp:inMargin left="0" right="0" top="0" bottom="0"/>"#);
        xml.push_str(&format!(
            r#"<hp:imgDim dimwidth="{}" dimheight="{}"/>"#,
            org_width, org_height
        ));
        xml.push_str("<hp:effects/>");

        // sz, pos, outMargin (참조 파일에서는 하단에 위치)
        xml.push_str(&format!(
            r#"<hp:sz width="{}" widthRelTo="ABSOLUTE" height="{}" heightRelTo="ABSOLUTE" protect="0"/>"#,
            cur_width, cur_height
        ));
        xml.push_str(concat!(
            r#"<hp:pos treatAsChar="1" affectLSpacing="0" flowWithText="1" allowOverlap="0" "#,
            r#"holdAnchorAndSO="0" vertRelTo="PARA" horzRelTo="PARA" vertAlign="TOP" "#,
            r#"horzAlign="LEFT" vertOffset="0" horzOffset="0"/>"#
        ));
        xml.push_str(r#"<hp:outMargin left="0" right="0" top="0" bottom="0"/>"#);
        xml.push_str("</hp:pic>");

        xml
    }

    fn format_table(&self, table: &HwpxTable) -> String {
        let row_cnt = table.rows.len();
        let col_cnt = table.rows.first().map(|r| r.len()).unwrap_or(0);
        if row_cnt == 0 || col_cnt == 0 {
            return String::new();
        }

        let content_width: u32 = 42520;
        let col_width = content_width / col_cnt as u32;
        let total_width = col_width * col_cnt as u32;
        let cell_height: u32 = 1000;

        let mut xml = format!(
            concat!(
                r#"<hp:tbl id="{}" zOrder="0" numberingType="TABLE" textWrap="TOP_AND_BOTTOM" "#,
                r#"textFlow="BOTH_SIDES" lock="0" dropcapstyle="None" pageBreak="CELL" "#,
                r#"repeatHeader="1" rowCnt="{}" colCnt="{}" cellSpacing="0" borderFillIDRef="3" noAdjust="0">"#,
                r#"<hp:sz width="{}" widthRelTo="ABSOLUTE" height="{}" heightRelTo="ABSOLUTE" protect="0"/>"#,
                r#"<hp:pos treatAsChar="0" affectLSpacing="0" flowWithText="1" allowOverlap="0" "#,
                r#"holdAnchorAndSO="0" vertRelTo="PARA" horzRelTo="PARA" vertAlign="TOP" "#,
                r#"horzAlign="LEFT" vertOffset="0" horzOffset="0"/>"#,
                r#"<hp:outMargin left="283" right="283" top="283" bottom="283"/>"#,
                r#"<hp:inMargin left="510" right="510" top="142" bottom="142"/>"#
            ),
            self.next_table_id, row_cnt, col_cnt, total_width,
            cell_height * row_cnt as u32
        );

        for row_idx in 0..row_cnt {
            xml.push_str("<hp:tr>");
            for col_idx in 0..col_cnt {
                // Skip cells covered by another cell's span
                if table.is_covered(row_idx, col_idx) {
                    continue;
                }

                let cell_text = &table.rows[row_idx][col_idx];
                let span = table.get_cell_span(row_idx, col_idx);
                let cell_w = col_width * span.col_span;
                let cell_h = cell_height * span.row_span;

                xml.push_str(&format!(
                    concat!(
                        r#"<hp:tc name="" header="0" hasMargin="0" protect="0" editable="0" dirty="0" borderFillIDRef="3">"#,
                        r#"<hp:subList id="" textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="CENTER" "#,
                        r#"linkListIDRef="0" linkListNextIDRef="0" textWidth="0" textHeight="0" hasTextRef="0" hasNumRef="0">"#,
                        r#"<hp:p id="0" paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0">"#,
                        r#"<hp:run charPrIDRef="0"><hp:t>{}</hp:t></hp:run>"#,
                        r#"</hp:p></hp:subList>"#,
                        r#"<hp:cellAddr colAddr="{}" rowAddr="{}"/>"#,
                        r#"<hp:cellSpan colSpan="{}" rowSpan="{}"/>"#,
                        r#"<hp:cellSz width="{}" height="{}"/>"#,
                        r#"<hp:cellMargin left="510" right="510" top="142" bottom="142"/>"#,
                        r#"</hp:tc>"#
                    ),
                    escape_xml(cell_text),
                    col_idx,
                    row_idx,
                    span.col_span,
                    span.row_span,
                    cell_w,
                    cell_h
                ));
            }
            xml.push_str("</hp:tr>");
        }

        xml.push_str("</hp:tbl>");
        xml
    }

    fn generate_header_ctrl_xml(&self) -> String {
        let mut xml = String::new();

        for (idx, header) in self.headers.iter().enumerate() {
            let apply_type = match header.apply_to {
                HeaderFooterApplyTo::All => "BOTH",
                HeaderFooterApplyTo::Odd => "ODD",
                HeaderFooterApplyTo::Even => "EVEN",
            };

            let content = if header.text.is_empty() {
                concat!(
                    r#"<hp:ctrl>"#,
                    r#"<hp:autoNum num="1" numType="PAGE">"#,
                    r#"<hp:autoNumFormat type="DIGIT" userChar="" prefixChar="" suffixChar="" supscript="0"/>"#,
                    r#"</hp:autoNum>"#,
                    r#"</hp:ctrl>"#,
                    r#"<hp:t/>"#
                ).to_string()
            } else {
                format!(r#"<hp:t>{}</hp:t>"#, escape_xml(&header.text))
            };

            xml.push_str(&format!(
                concat!(
                    r#"<hp:ctrl>"#,
                    r#"<hp:header id="{}" applyPageType="{}">"#,
                    r#"<hp:subList id="" textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="TOP" "#,
                    r#"linkListIDRef="0" linkListNextIDRef="0" textWidth="42520" textHeight="4252" "#,
                    r#"hasTextRef="0" hasNumRef="0">"#,
                    r#"<hp:p id="0" paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0">"#,
                    r#"<hp:run charPrIDRef="0">{}</hp:run>"#,
                    r#"</hp:p>"#,
                    r#"</hp:subList>"#,
                    r#"</hp:header>"#,
                    r#"</hp:ctrl>"#
                ),
                idx + 1,
                apply_type,
                content
            ));
        }

        xml
    }

    fn generate_footer_ctrl_xml(&self) -> String {
        let mut xml = String::new();

        for (idx, footer) in self.footers.iter().enumerate() {
            let apply_type = match footer.apply_to {
                HeaderFooterApplyTo::All => "BOTH",
                HeaderFooterApplyTo::Odd => "ODD",
                HeaderFooterApplyTo::Even => "EVEN",
            };

            let content = if footer.include_page_number {
                let format_type = footer.page_number_format.as_hwpx_format();
                format!(
                    concat!(
                        r#"<hp:t>{}</hp:t>"#,
                        r#"</hp:run><hp:run charPrIDRef="0">"#,
                        r#"<hp:ctrl>"#,
                        r#"<hp:autoNum num="1" numType="PAGE">"#,
                        r#"<hp:autoNumFormat type="{}" userChar="" prefixChar="" suffixChar="" supscript="0"/>"#,
                        r#"</hp:autoNum>"#,
                        r#"</hp:ctrl>"#,
                        r#"<hp:t/>"#
                    ),
                    escape_xml(&footer.text),
                    format_type
                )
            } else {
                format!(r#"<hp:t>{}</hp:t>"#, escape_xml(&footer.text))
            };

            xml.push_str(&format!(
                concat!(
                    r#"<hp:ctrl>"#,
                    r#"<hp:footer id="{}" applyPageType="{}">"#,
                    r#"<hp:subList id="" textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="TOP" "#,
                    r#"linkListIDRef="0" linkListNextIDRef="0" textWidth="42520" textHeight="4252" "#,
                    r#"hasTextRef="0" hasNumRef="0">"#,
                    r#"<hp:p id="0" paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0">"#,
                    r#"<hp:run charPrIDRef="0">{}</hp:run>"#,
                    r#"</hp:p>"#,
                    r#"</hp:subList>"#,
                    r#"</hp:footer>"#,
                    r#"</hp:ctrl>"#
                ),
                idx + 1,
                apply_type,
                content
            ));
        }

        xml
    }

    fn get_section_count(&self) -> usize {
        if self.document.body_texts.is_empty() {
            1
        } else {
            self.document
                .body_texts
                .iter()
                .flat_map(|b| &b.sections)
                .count()
                .max(1)
        }
    }
}

impl Default for HwpxWriter {
    fn default() -> Self {
        Self::new()
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hwpx_writer_new() {
        let writer = HwpxWriter::new();
        assert!(writer.document.body_texts.is_empty());
    }

    #[test]
    fn test_hwpx_writer_add_paragraph() {
        let mut writer = HwpxWriter::new();
        writer.add_paragraph("Hello").unwrap();
        writer.add_paragraph("World").unwrap();

        assert_eq!(writer.document.body_texts.len(), 1);
        assert_eq!(
            writer.document.body_texts[0].sections[0].paragraphs.len(),
            2
        );
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("\"quote\""), "&quot;quote&quot;");
    }

    #[test]
    fn test_generate_version_xml() {
        let writer = HwpxWriter::new();
        let xml = writer.generate_version_xml();
        assert!(xml.contains("HCFVersion"));
        assert!(xml.contains("2011/version"));
        assert!(xml.contains("standalone=\"yes\""));
        assert!(xml.contains("tagetApplication"));
    }
}
