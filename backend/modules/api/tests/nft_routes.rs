// BE-154: NFT endpoints are reachable under /v1/nft.
use actix_web::{test, App};

#[actix_web::test]
async fn nft_routes_are_registered() {
    let app = test::init_service(App::new().configure(st_core::endpoint::configure)).await;

    for uri in ["/nft/mint", "/nft/metadata/format"] {
        let req = test::TestRequest::post().uri(uri).to_request();
        let res = test::call_service(&app, req).await;
        // 400 means the route matched and deserialization rejected the empty
        // body; 404 would mean the route was never mounted.
        assert_ne!(res.status().as_u16(), 404, "{uri} should be mounted");
    }

    let req = test::TestRequest::get().uri("/nft/stellar-toml").to_request();
    let res = test::call_service(&app, req).await;
    assert_ne!(res.status().as_u16(), 404, "stellar-toml should be mounted");
}
