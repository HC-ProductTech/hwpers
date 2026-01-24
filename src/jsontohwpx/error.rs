use thiserror::Error;

/// jsontohwpx 변환 과정에서 발생할 수 있는 에러 타입
#[derive(Debug, Error)]
pub enum JsonToHwpxError {
    /// 입력 JSON 파싱 또는 검증 실패 (exit code 1)
    #[error("입력 에러: {0}")]
    Input(String),

    /// 변환 과정 에러 - 이미지 다운로드 실패, 포맷 변환 실패 등 (exit code 2)
    #[error("변환 에러: {0}")]
    Conversion(String),

    /// 파일 IO 에러 (exit code 3)
    #[error("IO 에러: {0}")]
    Io(#[from] std::io::Error),

    /// HwpxWriter 내부 에러
    #[error("HWPX 에러: {0}")]
    Hwpx(String),
}

impl JsonToHwpxError {
    /// CLI 종료 코드 반환
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Input(_) => 1,
            Self::Conversion(_) => 2,
            Self::Io(_) => 3,
            Self::Hwpx(_) => 2,
        }
    }

    /// 에러 코드 문자열 반환 (JSON 출력용)
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Input(_) => "INPUT_ERROR",
            Self::Conversion(_) => "CONVERSION_ERROR",
            Self::Io(_) => "IO_ERROR",
            Self::Hwpx(_) => "HWPX_ERROR",
        }
    }
}

impl From<crate::error::HwpError> for JsonToHwpxError {
    fn from(err: crate::error::HwpError) -> Self {
        Self::Hwpx(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, JsonToHwpxError>;
