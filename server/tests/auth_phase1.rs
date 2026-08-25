use messenger_server::auth_service::AuthServiceImpl;
use messenger_server::messenger::auth_service_server::AuthService;
use messenger_server::messenger::{RefreshTokenRequest, RequestOtpRequest, VerifyOtpRequest};
use tonic::Request;

mod common;
use common::{clean_db, test_config, test_pool};

fn svc_with_mock(pool: sqlx::PgPool, sms_mock: bool) -> AuthServiceImpl {
    let (secret, jwt_exp, otp_ttl, prefix) = test_config();
    AuthServiceImpl::new(pool, secret, jwt_exp, otp_ttl, sms_mock, prefix)
}

// Helper to get debug OTP via RequestOTP
async fn request_otp_get_code(svc: &AuthServiceImpl, phone: &str) -> String {
    let req = Request::new(RequestOtpRequest { phone: phone.to_string() });
    let resp = svc.request_otp(req).await.expect("request_otp").into_inner();
    assert!(resp.success);
    resp.debug_otp
}

#[tokio::test]
async fn test_request_otp_mock() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool.clone(), true);
    let phone = "+79990001111";
    let code = request_otp_get_code(&svc, phone).await;
    assert_eq!(code.len(), 6);
    assert!(code.chars().all(|c| c.is_ascii_digit()));
    // DB row exists
    let row: (String, i32, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT otp_hash, attempts, expires_at FROM phone_verification WHERE phone=$1",
    )
    .bind(phone)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.1, 0);
    assert!(row.2 > chrono::Utc::now());
    // bcrypt verifies
    assert!(bcrypt::verify(&code, &row.0).unwrap());
}

#[tokio::test]
async fn test_request_otp_no_mock() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool.clone(), false);
    let req = Request::new(RequestOtpRequest { phone: "+79990001112".into() });
    let resp = svc.request_otp(req).await.unwrap().into_inner();
    assert!(resp.success);
    assert!(resp.debug_otp.is_empty());
}

#[tokio::test]
async fn test_request_otp_invalid_phone() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool, true);
    let req = Request::new(RequestOtpRequest { phone: "123".into() });
    let err = svc.request_otp(req).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_request_otp_rate_limit_db() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool, true);
    let phone = "+79990001113";
    for _ in 0..3 {
        svc.request_otp(Request::new(RequestOtpRequest { phone: phone.into() }))
            .await
            .unwrap();
    }
    let err = svc
        .request_otp(Request::new(RequestOtpRequest { phone: phone.into() }))
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::ResourceExhausted);
}

#[tokio::test]
async fn test_verify_otp_new_user() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool.clone(), true);
    let phone = "+79990001114";
    let code = request_otp_get_code(&svc, phone).await;
    let resp = svc
        .verify_otp(Request::new(VerifyOtpRequest {
            phone: phone.into(),
            code: code.clone(),
            username: "".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(!resp.token.is_empty());
    assert!(resp.user.is_some());
    let user = resp.user.unwrap();
    assert_eq!(user.phone, phone);
    // phone_verification cleaned
    let cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM phone_verification WHERE phone=$1")
        .bind(phone)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt.0, 0);
    // user and profile exist
    let cnt: (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM "user" WHERE phone=$1"#)
        .bind(phone)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt.0, 1);
    let cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_profile WHERE user_id=$1")
        .bind(uuid::Uuid::parse_str(&user.id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt.0, 1);
    // JWT verifies
    let claims = messenger_server::auth::verify_token("integration-test-secret-32chars-long!!", &resp.token).unwrap();
    assert_eq!(claims.sub, user.id);
}

#[tokio::test]
async fn test_verify_otp_existing_user_reuse() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool.clone(), true);
    let phone = "+79990001115";
    let code1 = request_otp_get_code(&svc, phone).await;
    let r1 = svc
        .verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code: code1, username: "".into() }))
        .await
        .unwrap()
        .into_inner();
    let id1 = r1.user.unwrap().id;
    let code2 = request_otp_get_code(&svc, phone).await;
    let r2 = svc
        .verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code: code2, username: "".into() }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(id1, r2.user.unwrap().id);
}

#[tokio::test]
async fn test_verify_otp_with_username() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool.clone(), true);
    let phone = "+79990001116";
    let code = request_otp_get_code(&svc, phone).await;
    let resp = svc
        .verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code, username: "alice".into() }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.user.unwrap().username, "alice");
    // duplicate username -> suffix
    let phone2 = "+79990001117";
    let code2 = request_otp_get_code(&svc, phone2).await;
    let resp2 = svc
        .verify_otp(Request::new(VerifyOtpRequest { phone: phone2.into(), code: code2, username: "alice".into() }))
        .await
        .unwrap()
        .into_inner();
    assert_ne!(resp2.user.unwrap().username, "alice");
    assert!(resp2.user.unwrap().username.starts_with("alice_"));
}

#[tokio::test]
async fn test_verify_otp_wrong_code_increments() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool.clone(), true);
    let phone = "+79990001118";
    let _code = request_otp_get_code(&svc, phone).await;
    let err = svc
        .verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code: "000000".into(), username: "".into() }))
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
    let attempts: (i32,) = sqlx::query_as("SELECT attempts FROM phone_verification WHERE phone=$1")
        .bind(phone)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts.0, 1);
    // 5 wrong -> resource exhausted
    for _ in 0..4 {
        let _ = svc
            .verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code: "000000".into(), username: "".into() }))
            .await;
    }
    let err = svc
        .verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code: "000000".into(), username: "".into() }))
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::ResourceExhausted);
}

#[tokio::test]
async fn test_verify_otp_expired() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool.clone(), true);
    let phone = "+79990001119";
    let _code = request_otp_get_code(&svc, phone).await;
    // make expired
    sqlx::query("UPDATE phone_verification SET expires_at = NOW() - interval '1 second' WHERE phone=$1")
        .bind(phone)
        .execute(&pool)
        .await
        .unwrap();
    let err = svc
        .verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code: "000000".into(), username: "".into() }))
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
    assert!(err.message().contains("expired"));
}

#[tokio::test]
async fn test_verify_otp_replay_deleted() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool.clone(), true);
    let phone = "+79990001120";
    let code = request_otp_get_code(&svc, phone).await;
    svc.verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code: code.clone(), username: "".into() }))
        .await
        .unwrap();
    let err = svc
        .verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code, username: "".into() }))
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_verify_otp_6digit_validation() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool, true);
    for bad in ["123", "abc123", "1234567", ""] {
        let err = svc
            .verify_otp(Request::new(VerifyOtpRequest { phone: "+79990001121".into(), code: bad.into(), username: "".into() }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }
}

#[tokio::test]
async fn test_refresh_token_ok() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool, true);
    let phone = "+79990001122";
    let code = request_otp_get_code(&svc, phone).await;
    let r = svc
        .verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code, username: "".into() }))
        .await
        .unwrap()
        .into_inner();
    let old = r.token.clone();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let r2 = svc
        .refresh_token(Request::new(RefreshTokenRequest { token: old.clone() }))
        .await
        .unwrap()
        .into_inner();
    assert!(!r2.token.is_empty());
    assert_ne!(r2.token, old);
    assert_eq!(r2.user.unwrap().id, r.user.unwrap().id);
}

#[tokio::test]
async fn test_refresh_token_invalid() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool, true);
    let err = svc
        .refresh_token(Request::new(RefreshTokenRequest { token: "bad".into() }))
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_psql_cleanup() {
    let pool = test_pool().await;
    clean_db(&pool).await;
    let svc = svc_with_mock(pool.clone(), true);
    let phone = "+79990001123";
    let code = request_otp_get_code(&svc, phone).await;
    svc.verify_otp(Request::new(VerifyOtpRequest { phone: phone.into(), code, username: "".into() }))
        .await
        .unwrap();
    let cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM phone_verification WHERE phone=$1")
        .bind(phone)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt.0, 0);
}
