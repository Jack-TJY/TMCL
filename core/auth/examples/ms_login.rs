//! 用真实微软账号走一遍设备码登录全链路（M0-10 验收用）。
//!
//! `cargo run -p tmcl-auth --example ms_login`
//!
//! 需要构建时注入 `TMCL_MS_CLIENT_ID`。只输出登录代码、验证页、玩家名与令牌有效期，
//! 不输出任何令牌，也不保存账号。

use tmcl_auth::microsoft::MsClient;
use tmcl_auth::store;
use tmcl_common::{AppResult, TaskCtx};

#[tokio::main(flavor = "current_thread")]
async fn main() -> AppResult<()> {
    let ms = MsClient::from_build()?;
    let ctx = TaskCtx::noop("ms_login");
    let session = store::login(
        &ms,
        |code| {
            println!(
                "在浏览器打开 {} 并输入代码 {}（{} 秒内有效）",
                code.verification_uri, code.user_code, code.expires_in
            );
        },
        &ctx,
    )
    .await?;
    println!(
        "登录成功：{}（{}）",
        session.profile.name, session.profile.id
    );
    let refreshed = store::refresh(&ms, &session, &ctx).await?;
    println!(
        "刷新令牌续期成功：{}，访问令牌到期 unix {}",
        refreshed.profile.name, refreshed.expires_at
    );
    Ok(())
}
