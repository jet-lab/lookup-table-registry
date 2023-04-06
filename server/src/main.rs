use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::post;
use axum::{response::IntoResponse, Extension, Json, Router};
use lookup_table_registry_client::client::LookupRegistryClient;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    // Inject environment variables
    let _ = dotenv::dotenv();

    let solana_endpoint = std::env::var("SOLANA_ENDPOINT").unwrap();

    let context = ApiContext {
        registry_client: LookupRegistryClient::new(Arc::new(RpcClient::new(solana_endpoint))),
    };

    let app = Router::new()
        .route("/lookup/get_addresses", post(get_lookup_addresses))
        .layer(CorsLayer::permissive())
        .layer(Extension(context));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3006));
    tracing::info!("Listening on {addr}");
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

async fn get_lookup_addresses(
    Extension(context): Extension<ApiContext>,
    Json(input): Json<GetLookupAddressInput>,
) -> impl IntoResponse {
    // Refresh lookup addresses by authority
    context
        .registry_client
        .update_registries(&input.authorities)
        .await;
    let instructions = input
        .instructions
        .iter()
        .map(|ix| ix.into())
        .collect::<Vec<_>>();
    let result = context
        .registry_client
        .find_addresses(&instructions, &input.authorities);

    Json(GetAddressesResponse {
        distinct_accounts: result.distinct,
        unmatched_accounts: result.unmatched,
        addresses: result.matches,
    })
}

#[serde_as]
#[derive(Serialize, Deserialize)]
struct GetAddressesResponse {
    #[serde_as(as = "Vec<DisplayFromStr>")]
    addresses: Vec<Pubkey>,
    distinct_accounts: usize,
    unmatched_accounts: usize,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
struct GetLookupAddressInput {
    instructions: Vec<InstructionSmall>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    authorities: Vec<Pubkey>,
}

#[derive(Clone)]
struct ApiContext {
    registry_client: LookupRegistryClient,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
struct InstructionSmall {
    #[serde_as(as = "DisplayFromStr")]
    program: Pubkey,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    accounts: Vec<Pubkey>,
}

impl From<&InstructionSmall> for Instruction {
    fn from(val: &InstructionSmall) -> Self {
        Instruction {
            program_id: val.program,
            accounts: val
                .accounts
                .iter()
                .map(|acc| AccountMeta {
                    pubkey: *acc,
                    is_signer: false,
                    is_writable: false,
                })
                .collect(),
            data: vec![],
        }
    }
}
