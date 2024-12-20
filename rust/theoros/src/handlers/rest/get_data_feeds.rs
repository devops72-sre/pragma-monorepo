use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{ToResponse, ToSchema};

use pragma_feeds::Feed;

use crate::errors::GetDataFeedsError;
use crate::AppState;

#[derive(Debug, Default, Serialize, Deserialize, ToResponse, ToSchema)]
pub struct GetDataFeedsResponse(pub Vec<Feed>);

#[utoipa::path(
    get,
    path = "/v1/data_feeds",
    responses(
        (status = 200, description = "Get all the available feed ids", body = [GetDataFeedsResponse])
    ),
)]
pub async fn get_data_feeds(State(state): State<AppState>) -> Result<Json<GetDataFeedsResponse>, GetDataFeedsError> {
    let started_at = std::time::Instant::now();

    let feed_ids = state.storage.feed_ids();

    let mut feeds = Vec::with_capacity(feed_ids.len());
    for feed_id in feed_ids.iter() {
        let feed = feed_id.parse().map_err(|_| GetDataFeedsError::ParsingFeedId(feed_id.clone()))?;
        feeds.push(feed);
    }

    let response = GetDataFeedsResponse(feeds);
    tracing::info!("🌐 get_data_feeds - {:?}", started_at.elapsed());
    Ok(Json(response))
}
