//! Túnel remoto no computador: reexporta o núcleo compartilhado ([`dotcrow_tunel`]) e
//! acrescenta a [`ponte`], que conversa com o relay e faz o túnel da API do painel.

pub mod ponte;

pub use dotcrow_tunel::*;
