use std::path::Path;

use crate::hwpx::{HwpxImage, HwpxWriter};

use super::error::{JsonToHwpxError, Result};

/// 이미지 URL/경로에서 이미지를 로드하여 HwpxWriter에 추가
pub fn add_image_from_url(writer: &mut HwpxWriter, url: &str, base_path: &Path) -> Result<()> {
    let image_bytes = load_image_bytes(url, base_path)?;
    let image_bytes = convert_if_needed(image_bytes, url)?;

    let image = HwpxImage::from_bytes(image_bytes).ok_or_else(|| {
        JsonToHwpxError::Conversion(format!("지원하지 않는 이미지 포맷: {}", url))
    })?;

    writer.add_image(image)?;
    Ok(())
}

/// Base64 인코딩된 이미지를 디코딩하여 HwpxWriter에 추가
pub fn add_image_from_base64(
    writer: &mut HwpxWriter,
    data: &str,
    format: Option<&str>,
) -> Result<()> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| JsonToHwpxError::Conversion(format!("Base64 디코딩 실패: {}", e)))?;

    let bytes = convert_if_needed_by_format(bytes, format)?;

    let image = HwpxImage::from_bytes(bytes)
        .ok_or_else(|| JsonToHwpxError::Conversion("Base64 이미지 포맷 인식 실패".to_string()))?;

    writer.add_image(image)?;
    Ok(())
}

/// URL 또는 로컬 경로에서 이미지 바이트를 로드
fn load_image_bytes(url: &str, base_path: &Path) -> Result<Vec<u8>> {
    if url.starts_with("http://") || url.starts_with("https://") {
        download_image(url)
    } else {
        let path = base_path.join(url);
        std::fs::read(&path).map_err(|e| {
            JsonToHwpxError::Conversion(format!(
                "이미지 파일 읽기 실패: {} ({})",
                path.display(),
                e
            ))
        })
    }
}

/// 외부 URL에서 이미지 다운로드 (타임아웃 60초)
fn download_image(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| JsonToHwpxError::Conversion(format!("HTTP 클라이언트 생성 실패: {}", e)))?;

    let response = client.get(url).send().map_err(|e| {
        JsonToHwpxError::Conversion(format!("이미지 다운로드 실패: {} ({})", url, e))
    })?;

    if !response.status().is_success() {
        return Err(JsonToHwpxError::Conversion(format!(
            "이미지 다운로드 실패: {} (HTTP {})",
            url,
            response.status()
        )));
    }

    response.bytes().map(|b| b.to_vec()).map_err(|e| {
        JsonToHwpxError::Conversion(format!("이미지 데이터 읽기 실패: {} ({})", url, e))
    })
}

/// 확장자 기반으로 변환이 필요한 포맷인지 확인 후 PNG로 변환
fn convert_if_needed(bytes: Vec<u8>, url: &str) -> Result<Vec<u8>> {
    let ext = Path::new(url)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        Some("webp") | Some("avif") => convert_to_png(&bytes),
        Some("gif") => extract_gif_first_frame(&bytes),
        _ => {
            // 매직 바이트로 추가 확인
            if is_webp(&bytes) || is_avif(&bytes) {
                convert_to_png(&bytes)
            } else {
                Ok(bytes)
            }
        }
    }
}

/// format 필드 기반으로 변환 필요 여부 확인
fn convert_if_needed_by_format(bytes: Vec<u8>, format: Option<&str>) -> Result<Vec<u8>> {
    match format.map(|f| f.to_lowercase()).as_deref() {
        Some("webp") | Some("avif") => convert_to_png(&bytes),
        Some("gif") => extract_gif_first_frame(&bytes),
        _ => Ok(bytes),
    }
}

/// image 크레이트를 사용하여 PNG로 변환
fn convert_to_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| JsonToHwpxError::Conversion(format!("이미지 포맷 변환 실패: {}", e)))?;

    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| JsonToHwpxError::Conversion(format!("PNG 인코딩 실패: {}", e)))?;

    Ok(png_bytes)
}

/// GIF에서 첫 프레임 추출 → PNG로 변환
fn extract_gif_first_frame(bytes: &[u8]) -> Result<Vec<u8>> {
    // image 크레이트가 GIF 첫 프레임을 자동으로 로드
    convert_to_png(bytes)
}

/// WebP 매직 바이트 확인
fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

/// AVIF 매직 바이트 확인 (ftyp box)
fn is_avif(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[4..8] == b"ftyp" && &bytes[8..12] == b"avif"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_webp() {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(b"RIFF");
        data[8..12].copy_from_slice(b"WEBP");
        assert!(is_webp(&data));
    }

    #[test]
    fn test_is_not_webp() {
        let data = vec![0u8; 12];
        assert!(!is_webp(&data));
    }

    #[test]
    fn test_is_avif() {
        let mut data = vec![0u8; 12];
        data[4..8].copy_from_slice(b"ftyp");
        data[8..12].copy_from_slice(b"avif");
        assert!(is_avif(&data));
    }
}
