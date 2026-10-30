use crate::core::BrokerError;
use crate::mapping::{
    map_market_session_response, map_quote_response, map_stock_full_name_response,
};
use crate::protocol::{
    DOMESTIC_STOCK_HOLIDAY_ENDPOINT, DOMESTIC_STOCK_QUOTE_ENDPOINT,
    domestic_stock_search_stock_info_endpoint,
};
use crate::{BrokerHealth, MarketSession, Quote, Symbol};

use super::KisBrokerClient;

impl KisBrokerClient {
    pub async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        let mut quote = self.get_quote_price(symbol).await?;
        match self.get_domestic_stock_full_name(symbol).await {
            Ok(full_name) => quote.full_name = full_name,
            Err(error) => {
                tracing::warn!(
                    symbol = symbol.as_str(),
                    error = %error,
                    "KIS stock full-name enrichment failed"
                );
            }
        }
        Ok(quote)
    }

    async fn get_quote_price(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                DOMESTIC_STOCK_QUOTE_ENDPOINT,
                &[
                    ("FID_COND_MRKT_DIV_CODE", "J"),
                    ("FID_INPUT_ISCD", symbol.as_str()),
                ],
                None,
            )?)
            .await?;

        map_quote_response(response.body())
    }

    pub async fn get_domestic_stock_full_name(
        &self,
        symbol: &Symbol,
    ) -> Result<Option<String>, BrokerError> {
        let endpoint = domestic_stock_search_stock_info_endpoint(self.transport.environment())?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                endpoint,
                &[("PRDT_TYPE_CD", "300"), ("PDNO", symbol.as_str())],
                None,
            )?)
            .await?;

        map_stock_full_name_response(response.body())
    }

    pub async fn get_market_session(
        &self,
        trading_day: &str,
    ) -> Result<MarketSession, BrokerError> {
        if trading_day.len() != 8
            || !trading_day
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            return Err(BrokerError::validation(
                "trading_day must be formatted as YYYYMMDD",
            ));
        }

        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                DOMESTIC_STOCK_HOLIDAY_ENDPOINT,
                &[
                    ("BASS_DT", trading_day),
                    ("CTX_AREA_FK", ""),
                    ("CTX_AREA_NK", ""),
                ],
                None,
            )?)
            .await?;

        map_market_session_response(response.body())
    }

    pub async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        let symbol = Symbol::new("005930")?;
        self.get_quote_price(&symbol).await?;
        Ok(BrokerHealth {
            environment: self.transport.environment(),
            writable: false,
        })
    }
}
