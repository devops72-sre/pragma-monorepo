use alloy::primitives::hex;
use anyhow::{Context, Result};
use pragma_feeds::FeedType;
use starknet::core::types::{Felt, U256};

use pragma_utils::conversions::apibara::FromFieldBytes;

use super::FromStarknetEventData;

const MESSAGE_HEADER_FELT_SIZE: usize = 10;
const SPOT_MEDIAN_UPDATE_SIZE: usize = 107;

#[derive(Debug, Clone)]
pub struct DispatchEvent {
    #[allow(unused)]
    pub sender: U256,
    #[allow(unused)]
    pub destination_domain: u32,
    #[allow(unused)]
    pub recipient_address: U256,
    pub message: DispatchMessage,
}

// Creates a Dispatch from a Dispatch starknet event data, which is:
// 0. sender address
// 1. destination chain id
// 2. recipient address
// 3. message
//    a. header =>
//        - version,
//        - nonce,
//        - origin,
//        - sender_low,
//        - sender_high,
//        - destination,
//        - recipient_low,
//        - recipient_high,
//    b. body:
//        - nbr data_feeds updated
//        - update (per data_feed) =>
//            - asset_class
//            - data_type (given it, we know update_size)
//            [depending on the asset_class <=> data_type tuple, update below...]
//            [for example for SpotMedian below]
//            - pair_id
//            - price
//            - volume
//            - decimals
//            - timestamp
//            - sources_aggregated
impl FromStarknetEventData for DispatchEvent {
    fn from_starknet_event_data(data: Vec<Felt>) -> Result<Self> {
        let mut data = data.iter();

        let sender = U256::from_words(
            u128::from_field_bytes(data.next().context("Missing sender part 1")?.to_bytes_be()),
            u128::from_field_bytes(data.next().context("Missing sender part 2")?.to_bytes_be()),
        );

        let destination_domain = u32::from_field_bytes(data.next().context("Missing destination")?.to_bytes_be());

        let recipient_address = U256::from_words(
            u128::from_field_bytes(data.next().context("Missing recipient part 1")?.to_bytes_be()),
            u128::from_field_bytes(data.next().context("Missing recipient part 2")?.to_bytes_be()),
        );

        let header = DispatchMessageHeader::from_starknet_event_data(data.clone().cloned().collect())?;
        let body_data: Vec<Felt> = data.skip(MESSAGE_HEADER_FELT_SIZE).cloned().collect();
        let body = DispatchMessageBody::from_starknet_event_data(body_data)?;

        let message = DispatchMessage { header, body };

        let dispatch = Self { sender, destination_domain, recipient_address, message };
        Ok(dispatch)
    }
}

#[derive(Debug, Clone)]
pub struct DispatchMessage {
    pub header: DispatchMessageHeader,
    pub body: DispatchMessageBody,
}

#[derive(Debug, Clone)]
pub struct DispatchMessageHeader {
    #[allow(unused)]
    pub version: u8,
    pub nonce: u32,
    pub origin: u32,
    pub sender: U256,
    #[allow(unused)]
    pub destination: u32,
    #[allow(unused)]
    pub recipient: U256,
}

impl FromStarknetEventData for DispatchMessageHeader {
    fn from_starknet_event_data(data: Vec<Felt>) -> Result<Self> {
        let mut data = data.iter();
        Ok(Self {
            version: u8::from_field_bytes(data.next().context("Missing version")?.to_bytes_be()),
            nonce: u32::from_field_bytes(data.next().context("Missing nonce")?.to_bytes_be()),
            origin: u32::from_field_bytes(data.next().context("Missing origin")?.to_bytes_be()),
            sender: U256::from_words(
                u128::from_field_bytes(data.next().context("Missing sender part 1")?.to_bytes_be()),
                u128::from_field_bytes(data.next().context("Missing sender part 2")?.to_bytes_be()),
            ),
            destination: u32::from_field_bytes(data.next().context("Missing destination")?.to_bytes_be()),
            recipient: U256::from_words(
                u128::from_field_bytes(data.next().context("Missing recipient part 1")?.to_bytes_be()),
                u128::from_field_bytes(data.next().context("Missing recipient part 2")?.to_bytes_be()),
            ),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DispatchMessageBody {
    #[allow(unused)]
    pub nb_updated: u8,
    pub updates: Vec<DispatchUpdate>,
}

impl FromStarknetEventData for DispatchMessageBody {
    fn from_starknet_event_data(data: Vec<Felt>) -> Result<Self> {
        // Flatten the Felt data by removing the first 32 bytes and concatenating the rest
        let mut data: Vec<u8> = data
            .iter()
            .flat_map(|fe| {
                let bytes = fe.to_bytes_be();
                // Skip the first 32 bytes, if applicable
                bytes.split_at(16).1.to_vec()
            })
            .collect();

        let nb_updated = u8::from_be_bytes(data.drain(..1).collect::<Vec<u8>>().try_into().unwrap());
        let mut updates = Vec::with_capacity(nb_updated as usize);

        for _ in 0..nb_updated {
            let update = DispatchUpdate::from_starknet_event_data(data.clone()).context("Failed to parse update")?;
            match update {
                DispatchUpdate::SpotMedian { update: _, feed_id: _ } => {
                    data.drain(..SPOT_MEDIAN_UPDATE_SIZE);
                }
            }
            updates.push(update);
        }

        // Check that the number of parsed updates matches the declared number
        if updates.len() != nb_updated as usize {
            anyhow::bail!(
                "Mismatch between declared number of updates ({}) and actual updates parsed ({})",
                nb_updated,
                updates.len()
            );
        }

        // Return the result
        Ok(Self { nb_updated, updates })
    }
}

#[derive(Debug, Clone)]
pub struct DispatchUpdateInfos {
    pub nonce: u32,
    pub emitter_chain_id: u32,
    pub emitter_address: Felt,
    pub update: DispatchUpdate,
}

impl DispatchUpdateInfos {
    pub fn new(event: &DispatchEvent, update: &DispatchUpdate) -> Self {
        DispatchUpdateInfos {
            nonce: event.message.header.nonce,
            emitter_chain_id: event.message.header.origin,
            emitter_address: Felt::from_dec_str(&event.message.header.sender.to_string()).unwrap(),
            update: update.clone(),
        }
    }
}

// TODO: Should be a trait?
#[derive(Debug, Clone)]
pub enum DispatchUpdate {
    SpotMedian { update: SpotMedianUpdate, feed_id: String },
}

impl DispatchUpdate {
    pub fn feed_id(&self) -> String {
        match self {
            DispatchUpdate::SpotMedian { feed_id, update: _ } => feed_id.clone(),
        }
    }

    fn from_starknet_event_data(mut data: Vec<u8>) -> Result<Self> {
        let raw_asset_class = u16::from_be_bytes(data.drain(..2).collect::<Vec<u8>>().try_into().unwrap());

        let raw_feed_type = u16::from_be_bytes(data.drain(..2).collect::<Vec<u8>>().try_into().unwrap());
        let feed_type = FeedType::try_from(raw_feed_type)?;

        let pair_id_high = u128::from_be_bytes(data.drain(..16).collect::<Vec<u8>>().try_into().unwrap());
        let mut padded_data = [0u8; 16];
        let extracted_data = data.drain(..12).collect::<Vec<u8>>();
        padded_data[4..].copy_from_slice(&extracted_data);
        let pair_id_low = u128::from_be_bytes(padded_data);
        let pair_id = U256::from_words(pair_id_low, pair_id_high);

        let feed_id = build_feed_id(raw_asset_class, raw_feed_type, pair_id_high, pair_id_low);

        let update = match feed_type {
            FeedType::UniqueSpotMedian => {
                let mut res = SpotMedianUpdate::from_starknet_event_data(data)?;
                res.pair_id = pair_id;
                DispatchUpdate::SpotMedian { update: res, feed_id }
            }
        };

        Ok(update)
    }
}

fn build_feed_id(raw_asset_class: u16, raw_feed_type: u16, pair_id_high: u128, pair_id_low: u128) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&raw_asset_class.to_be_bytes());
    bytes.extend_from_slice(&raw_feed_type.to_be_bytes());
    bytes.extend_from_slice(&pair_id_high.to_be_bytes());
    bytes.extend_from_slice(&pair_id_low.to_be_bytes());
    let feed_id = format!("0x{}", hex::encode(&bytes));
    feed_id
}

#[derive(Debug, Clone)]
pub struct MetadataUpdate {
    pub timestamp: u64,
    pub num_sources_aggregated: u16,
    pub decimals: u8,
}

#[derive(Debug, Clone)]
pub struct SpotMedianUpdate {
    pub pair_id: U256,
    pub metadata: MetadataUpdate,
    pub price: U256,
    pub volume: U256,
}

impl SpotMedianUpdate {
    fn from_starknet_event_data(mut data: Vec<u8>) -> Result<Self> {
        let timestamp = u64::from_be_bytes(data.drain(..8).collect::<Vec<u8>>().try_into().unwrap());
        let num_sources_aggregated = u16::from_be_bytes(data.drain(..2).collect::<Vec<u8>>().try_into().unwrap());
        let decimals = u8::from_be_bytes(data.drain(..1).collect::<Vec<u8>>().try_into().unwrap());
        let price_high = u128::from_be_bytes(data.drain(..16).collect::<Vec<u8>>().try_into().unwrap()); // U256
        let price_low = u128::from_be_bytes(data.drain(..16).collect::<Vec<u8>>().try_into().unwrap());
        let price = U256::from_words(price_low, price_high);
        let volume_high = u128::from_be_bytes(data.drain(..16).collect::<Vec<u8>>().try_into().unwrap()); // U256
        let volume_low = u128::from_be_bytes(data.drain(..16).collect::<Vec<u8>>().try_into().unwrap());
        let volume = U256::from_words(volume_low, volume_high);

        Ok(Self {
            pair_id: U256::from(0_u8), // This will get populated later
            metadata: MetadataUpdate { decimals, timestamp, num_sources_aggregated },
            price,
            volume,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(&self.pair_id.low().to_be_bytes());
        bytes.extend_from_slice(&self.pair_id.high().to_be_bytes());

        bytes.extend_from_slice(&self.metadata.timestamp.to_be_bytes());
        bytes.extend_from_slice(&self.metadata.num_sources_aggregated.to_be_bytes());
        bytes.extend_from_slice(&self.metadata.decimals.to_be_bytes());

        bytes.extend_from_slice(&self.price.high().to_be_bytes());
        bytes.extend_from_slice(&self.price.low().to_be_bytes());

        bytes.extend_from_slice(&self.volume.high().to_be_bytes());
        bytes.extend_from_slice(&self.volume.low().to_be_bytes());
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_event_data(raw_data: Vec<&str>) -> Vec<Felt> {
        raw_data.iter().map(|hex_str| Felt::from_hex(hex_str).unwrap()).collect()
    }

    // TODO: Fix this test!
    #[test]
    fn test_dispatch_event_from_event_data() {
        let event_data = create_event_data(vec![
            "0x00000000000000000000000000000000e12de834144d9e90044ac03f6024267e",
            "0x0000000000000000000000000000000004d997c57f63d509f483927ce74135a4",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000003",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000611a3d",
            "0x00000000000000000000000000000000e12de834144d9e90044ac03f6024267e",
            "0x0000000000000000000000000000000004d997c57f63d509f483927ce74135a4",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x00000000000000000000000000000000000000000000000000000000000000d8",
            "0x000000000000000000000000000000000000000000000000000000000000000e",
            "0x0000000000000000000000000000000000020000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000004254432f55",
            "0x0000000000000000000000000000000053440000000067094ce4000108000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x000000000000000000000000000000000000000000000005a9d39c70a7000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x000000000000000000000000000000000000000000004554482f555344000000",
            "0x000000000000000000000000000000000067094ce40001080000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000038f1e274c20000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000000000000000000000000000",
        ]);

        let dispatch_event = DispatchEvent::from_starknet_event_data(event_data).unwrap();

        // assert_eq!(dispatch_event.sender, U256::from_words(299314662055416172851006310266400155262, 0)); // ça
        // assert_eq!(dispatch_event.destination_domain, 0);
        // assert_eq!(dispatch_event.recipient_address, U256::from(0_u32));

        // let header = &dispatch_event.message.header;
        // assert_eq!(header.version, 1);
        // assert_eq!(header.nonce, 4);
        // assert_eq!(header.origin, 5);
        // assert_eq!(header.sender, U256::from_words(299314662055416172851006310266400155262, 0));
        // assert_eq!(header.destination, 7);
        // assert_eq!(header.recipient, U256::from(0_u32));

        let body = &dispatch_event.message.body;
        assert_eq!(body.nb_updated, 2);
        assert_eq!(body.updates.len(), 2);

        // match &body.updates[0] {
        //     DispatchUpdate::SpotMedian { feed_id: _, update } => {
        //         assert_eq!(update.pair_id, U256::from(9_u32));
        //         assert_eq!(update.price, BigDecimal::from(10)); // 1000 / 10^2
        //         assert_eq!(update.volume, 0_u128);
        //         assert_eq!(update.metadata.decimals, 2);
        //         assert_eq!(update.metadata.timestamp, 1234567890);
        //         assert_eq!(update.metadata.num_sources_aggregated, 5);
        //     }
        // }
    }
}
