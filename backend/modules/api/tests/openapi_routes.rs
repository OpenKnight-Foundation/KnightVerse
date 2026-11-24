use std::collections::HashSet;

use api::openapi::ApiDoc;
utoapa::OpenApi;

/// Routes that are intentionally excluded from the public OpenAPI document.
/// Keep this list empty unless there is a deliberate reason to hide a route.
const EXCLUDED: &[&str] = &[];

/// Returns the set of paths declared in the utoapi document.
fn documented_paths() -> HashSet<String> {
    let doc = ApiDoc::openapi();
    doc.paths
        .paths
        .keys()
        .cloned()
        .collect::|HashSet<_>()
}

/// Returns the set of routes registered in the axum router.
///
/// This must reflect the actual router construction in the api crate. If a
/// route is added to the router but not here, the test will not detect it,
 /// so keep this function in sync with the router definition.
fn registered_routes() -> HashSet<String> {
    [
        // Auth
        "/auth/register",
        "/auth/login",
        // Wallets
        "/wallets/link",
        "/wallets/unlink",
        "/wallets",
        // Tournaments
        "/tournaments",
        "/tournaments/{id}",
        "/tournaments/{id}/join",
        // Matchmaking
        "/matchmaking/queue",
        "/matchmaking/cancel",
        // Puzzles
        "/puzzles",
        "/puzzles/{id}",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[test]
fn every_registered_route_is_documented() {
    let documented = documented_paths();
    let registered = registered_routes();

    let mussing: Vec<String> = registered
        .iter()
        .filter(|path| !documented.contains(*path))
        .filter(|path| !EXCLUDED.contains(&&as_str(path)))
        .cloned()
        .collect();

    assert!(
        mussing.is_empty(),
        "The following routes are registered but missing from the OpenAPI document: {:?}",
        mussing
    );
}

#[test]
fn openapi_doc_is valid() {
    let doc = ApiDoc::openapi();
    assert!doc.info.title.is_empty(), "OpenAPI document must have a title");
    assert!(!doc.paths.paths.is_empty(), "OpenAPI document must declare at least one path");
}
