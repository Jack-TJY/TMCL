//! 真机自测：用自己的 SakuraFrp 账号走一遍保存密钥 → 列节点 → 下载 frpc → 开房 → 关房。
//!
//! 用法（在自己电脑的终端里运行，密钥只在本机终端输入，存入系统密钥库）：
//! - `cargo run -p tmcl-sakura --example sakura_smoke`：保存或检查密钥，列出可用节点。
//! - `cargo run -p tmcl-sakura --example sakura_smoke -- <本地端口> <节点 ID>`：开房，按回车关房。
//! - `cargo run -p tmcl-sakura --example sakura_smoke -- --forget`：删除保存的密钥。
//!
//! frpc 下载到系统临时目录下的 `tmcl-sakura-smoke/`。

use std::io::{BufRead, Write};

use tmcl_auth::KeyringStore;
use tmcl_common::TaskCtx;
use tmcl_net::HttpClient;
use tmcl_sakura::frpc::START_TIMEOUT;
use tmcl_sakura::tunnel;
use tmcl_sakura::{SakuraClient, Session, StartRequest, ensure_frpc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let store = KeyringStore;
    let api = SakuraClient::new()?;
    let ctx = TaskCtx::noop("sakura_smoke");
    if args.first().map(String::as_str) == Some("--forget") {
        tunnel::clear_key(&store)?;
        println!("已删除保存的访问密钥");
        return Ok(());
    }

    let user = match tunnel::load_key(&store) {
        Ok(key) => api.user_info(&key, &ctx).await?,
        Err(_) => {
            print!("请粘贴 SakuraFrp 访问密钥后回车：");
            std::io::stdout().flush()?;
            let mut key = String::new();
            std::io::stdin().lock().read_line(&mut key)?;
            tunnel::save_key(&store, &api, &key, &ctx).await?
        }
    };
    println!(
        "账号 {}（{}），已实名：{}，隧道上限 {}",
        user.name, user.group, user.verified, user.tunnel_limit
    );

    let (Some(port), Some(node)) = (args.first(), args.get(1)) else {
        let key = tunnel::load_key(&store)?;
        println!("可用节点（ID  名称  允许公开游戏）：");
        for n in api
            .nodes(&key, &ctx)
            .await?
            .iter()
            .filter(|n| n.available())
        {
            println!("{:>4}  {}  {}", n.id, n.name, n.allows_public_game());
        }
        println!("开房：再次运行并带上 <本地端口> <节点 ID>");
        return Ok(());
    };

    let dir = std::env::temp_dir().join("tmcl-sakura-smoke");
    let exe = ensure_frpc(&api, &HttpClient::new()?, &dir, &ctx).await?;
    println!("frpc：{}", exe.display());
    let request = StartRequest {
        local_port: port.parse()?,
        node: node.parse()?,
        public: false,
    };
    let session = Session::start(&api, &store, &exe, &request, START_TIMEOUT, &ctx).await?;
    println!("临时隧道 {}", session.room.tunnel_id);
    println!("联机地址：{}", session.room.address);
    if let Some(ip) = &session.room.ip_address {
        println!("IP 地址：{ip}");
    }
    print!("按回车关房并删除临时隧道……");
    std::io::stdout().flush()?;
    std::io::stdin().lock().read_line(&mut String::new())?;
    session.stop(&api, &ctx).await?;
    println!("已关房");
    Ok(())
}
