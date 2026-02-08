fn main() {
    // LICENSE_EXPIRY 환경변수가 빌드 시 설정되면 컴파일 타임 상수로 주입
    if let Ok(expiry) = std::env::var("LICENSE_EXPIRY") {
        println!("cargo:rustc-env=LICENSE_EXPIRY_EMBEDDED={}", expiry);
    }
    // LICENSE_EXPIRY가 변경되면 재빌드
    println!("cargo:rerun-if-env-changed=LICENSE_EXPIRY");
}
