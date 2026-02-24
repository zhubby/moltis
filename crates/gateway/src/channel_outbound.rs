use std::sync::Arc;

use {anyhow::Result, async_trait::async_trait, tokio::sync::RwLock};

use {
    moltis_channels::{ChannelOutbound, ChannelStreamOutbound, StreamReceiver},
    moltis_msteams::MsTeamsPlugin,
    moltis_telegram::TelegramPlugin,
};

enum OutboundKind {
    Telegram,
    MsTeams,
}

pub struct MultiChannelOutbound {
    telegram_plugin: Arc<RwLock<TelegramPlugin>>,
    msteams_plugin: Arc<RwLock<MsTeamsPlugin>>,
    telegram_outbound: Arc<dyn ChannelOutbound>,
    msteams_outbound: Arc<dyn ChannelOutbound>,
}

impl MultiChannelOutbound {
    pub fn new(
        telegram_plugin: Arc<RwLock<TelegramPlugin>>,
        msteams_plugin: Arc<RwLock<MsTeamsPlugin>>,
        telegram_outbound: Arc<dyn ChannelOutbound>,
        msteams_outbound: Arc<dyn ChannelOutbound>,
    ) -> Self {
        Self {
            telegram_plugin,
            msteams_plugin,
            telegram_outbound,
            msteams_outbound,
        }
    }

    async fn resolve_kind(&self, account_id: &str) -> Result<OutboundKind> {
        let (tg_has, ms_has) = {
            let tg = self.telegram_plugin.read().await;
            let ms = self.msteams_plugin.read().await;
            (tg.has_account(account_id), ms.has_account(account_id))
        };
        match (tg_has, ms_has) {
            (true, false) => Ok(OutboundKind::Telegram),
            (false, true) => Ok(OutboundKind::MsTeams),
            (true, true) => Err(anyhow::anyhow!(
                "account_id '{account_id}' exists in multiple channels; explicit channel routing required"
            )),
            (false, false) => Err(anyhow::anyhow!("unknown channel account: {account_id}")),
        }
    }
}

#[async_trait]
impl ChannelOutbound for MultiChannelOutbound {
    async fn send_text(
        &self,
        account_id: &str,
        to: &str,
        text: &str,
        reply_to: Option<&str>,
    ) -> Result<()> {
        match self.resolve_kind(account_id).await? {
            OutboundKind::Telegram => {
                self.telegram_outbound
                    .send_text(account_id, to, text, reply_to)
                    .await
            },
            OutboundKind::MsTeams => {
                self.msteams_outbound
                    .send_text(account_id, to, text, reply_to)
                    .await
            },
        }
    }

    async fn send_media(
        &self,
        account_id: &str,
        to: &str,
        payload: &moltis_common::types::ReplyPayload,
        reply_to: Option<&str>,
    ) -> Result<()> {
        match self.resolve_kind(account_id).await? {
            OutboundKind::Telegram => {
                self.telegram_outbound
                    .send_media(account_id, to, payload, reply_to)
                    .await
            },
            OutboundKind::MsTeams => {
                self.msteams_outbound
                    .send_media(account_id, to, payload, reply_to)
                    .await
            },
        }
    }

    async fn send_typing(&self, account_id: &str, to: &str) -> Result<()> {
        match self.resolve_kind(account_id).await? {
            OutboundKind::Telegram => self.telegram_outbound.send_typing(account_id, to).await,
            OutboundKind::MsTeams => self.msteams_outbound.send_typing(account_id, to).await,
        }
    }

    async fn send_text_with_suffix(
        &self,
        account_id: &str,
        to: &str,
        text: &str,
        suffix_html: &str,
        reply_to: Option<&str>,
    ) -> Result<()> {
        match self.resolve_kind(account_id).await? {
            OutboundKind::Telegram => {
                self.telegram_outbound
                    .send_text_with_suffix(account_id, to, text, suffix_html, reply_to)
                    .await
            },
            OutboundKind::MsTeams => {
                self.msteams_outbound
                    .send_text_with_suffix(account_id, to, text, suffix_html, reply_to)
                    .await
            },
        }
    }

    async fn send_html(
        &self,
        account_id: &str,
        to: &str,
        html: &str,
        reply_to: Option<&str>,
    ) -> Result<()> {
        match self.resolve_kind(account_id).await? {
            OutboundKind::Telegram => {
                self.telegram_outbound
                    .send_html(account_id, to, html, reply_to)
                    .await
            },
            OutboundKind::MsTeams => {
                self.msteams_outbound
                    .send_html(account_id, to, html, reply_to)
                    .await
            },
        }
    }

    async fn send_text_silent(
        &self,
        account_id: &str,
        to: &str,
        text: &str,
        reply_to: Option<&str>,
    ) -> Result<()> {
        match self.resolve_kind(account_id).await? {
            OutboundKind::Telegram => {
                self.telegram_outbound
                    .send_text_silent(account_id, to, text, reply_to)
                    .await
            },
            OutboundKind::MsTeams => {
                self.msteams_outbound
                    .send_text_silent(account_id, to, text, reply_to)
                    .await
            },
        }
    }

    async fn send_location(
        &self,
        account_id: &str,
        to: &str,
        latitude: f64,
        longitude: f64,
        title: Option<&str>,
        reply_to: Option<&str>,
    ) -> Result<()> {
        match self.resolve_kind(account_id).await? {
            OutboundKind::Telegram => {
                self.telegram_outbound
                    .send_location(account_id, to, latitude, longitude, title, reply_to)
                    .await
            },
            OutboundKind::MsTeams => {
                self.msteams_outbound
                    .send_location(account_id, to, latitude, longitude, title, reply_to)
                    .await
            },
        }
    }
}

pub struct MultiChannelStreamOutbound {
    telegram_plugin: Arc<RwLock<TelegramPlugin>>,
    msteams_plugin: Arc<RwLock<MsTeamsPlugin>>,
    telegram_stream: Arc<dyn ChannelStreamOutbound>,
    msteams_stream: Arc<dyn ChannelStreamOutbound>,
}

impl MultiChannelStreamOutbound {
    pub fn new(
        telegram_plugin: Arc<RwLock<TelegramPlugin>>,
        msteams_plugin: Arc<RwLock<MsTeamsPlugin>>,
        telegram_stream: Arc<dyn ChannelStreamOutbound>,
        msteams_stream: Arc<dyn ChannelStreamOutbound>,
    ) -> Self {
        Self {
            telegram_plugin,
            msteams_plugin,
            telegram_stream,
            msteams_stream,
        }
    }

    async fn resolve_kind(&self, account_id: &str) -> Result<OutboundKind> {
        let (tg_has, ms_has) = {
            let tg = self.telegram_plugin.read().await;
            let ms = self.msteams_plugin.read().await;
            (tg.has_account(account_id), ms.has_account(account_id))
        };
        match (tg_has, ms_has) {
            (true, false) => Ok(OutboundKind::Telegram),
            (false, true) => Ok(OutboundKind::MsTeams),
            (true, true) => Err(anyhow::anyhow!(
                "account_id '{account_id}' exists in multiple channels; explicit channel routing required"
            )),
            (false, false) => Err(anyhow::anyhow!("unknown channel account: {account_id}")),
        }
    }
}

#[async_trait]
impl ChannelStreamOutbound for MultiChannelStreamOutbound {
    async fn send_stream(
        &self,
        account_id: &str,
        to: &str,
        reply_to: Option<&str>,
        stream: StreamReceiver,
    ) -> Result<()> {
        match self.resolve_kind(account_id).await? {
            OutboundKind::Telegram => {
                self.telegram_stream
                    .send_stream(account_id, to, reply_to, stream)
                    .await
            },
            OutboundKind::MsTeams => {
                self.msteams_stream
                    .send_stream(account_id, to, reply_to, stream)
                    .await
            },
        }
    }

    async fn is_stream_enabled(&self, account_id: &str) -> bool {
        match self.resolve_kind(account_id).await {
            Ok(OutboundKind::Telegram) => self.telegram_stream.is_stream_enabled(account_id).await,
            Ok(OutboundKind::MsTeams) => self.msteams_stream.is_stream_enabled(account_id).await,
            Err(_) => false,
        }
    }
}
