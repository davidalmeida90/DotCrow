//! Tipos centrais do DotCrow: permissões, níveis de risco, ações e a política que
//! decide quando o dono do computador precisa confirmar algo.
//!
//! Regra de ouro: **o DotCrow nunca faz em silêncio algo que não dá para desfazer.**

use serde::{Deserialize, Serialize};

/// O que uma ação precisa poder fazer no computador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Ler arquivos, janelas e telas dentro das pastas liberadas.
    Read,
    /// Criar ou alterar arquivos dentro das pastas liberadas.
    Write,
    /// Apagar arquivos (sempre via lixeira quando o sistema permitir).
    Delete,
    /// Rodar programas e comandos.
    Execute,
    /// Acessar a internet.
    Network,
    /// Mexer em configurações do sistema, instalar programas, usar privilégios de administrador.
    System,
}

impl Permission {
    /// Nome curto em português, usado na interface.
    pub fn label_pt(self) -> &'static str {
        match self {
            Permission::Read => "ler",
            Permission::Write => "escrever",
            Permission::Delete => "apagar",
            Permission::Execute => "executar",
            Permission::Network => "rede",
            Permission::System => "sistema",
        }
    }
}

/// Quanto estrago uma ação pode causar se der errado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Só leitura, nada muda.
    None,
    /// Muda algo, mas dá para desfazer facilmente.
    Low,
    /// Muda algo importante ou envia dados para fora.
    Medium,
    /// Não dá para desfazer, envolve dinheiro, credenciais ou o sistema.
    High,
}

/// Uma ação concreta que um agente quer executar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    /// Descrição legível, mostrada ao dono quando pedir confirmação.
    pub description: String,
    /// Permissões que a ação exige.
    pub permissions: Vec<Permission>,
    /// Risco estimado (o agente Vigia pode aumentar, nunca diminuir).
    pub risk: Risk,
    /// Se a ação pode ser desfeita automaticamente.
    pub reversible: bool,
}

/// O que fazer com uma ação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision", content = "reason")]
pub enum Decision {
    /// Pode seguir sem perguntar.
    Allow,
    /// Precisa da confirmação do dono antes.
    Confirm(String),
    /// Bloqueada pela política.
    Deny(String),
}

/// Política de permissões configurada pelo dono do computador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// Permissões liberadas sem confirmação (desde que o risco seja baixo).
    pub auto_allow: Vec<Permission>,
    /// Permissões proibidas, mesmo com confirmação.
    pub denied: Vec<Permission>,
    /// Risco máximo que pode rodar sem confirmação.
    pub max_unconfirmed_risk: Risk,
}

impl Default for Policy {
    /// Padrão conservador: só leitura roda sozinha; mexer no sistema é proibido até
    /// o dono liberar explicitamente.
    fn default() -> Self {
        Self { auto_allow: vec![Permission::Read], denied: vec![Permission::System], max_unconfirmed_risk: Risk::Low }
    }
}

impl Policy {
    /// Decide o que fazer com `action`.
    pub fn decide(&self, action: &Action) -> Decision {
        if let Some(p) = action.permissions.iter().find(|p| self.denied.contains(p)) {
            return Decision::Deny(format!("a permissão \"{}\" está bloqueada na política", p.label_pt()));
        }
        if !action.reversible {
            return Decision::Confirm("a ação não pode ser desfeita".into());
        }
        if action.risk > self.max_unconfirmed_risk {
            return Decision::Confirm(format!("risco {:?} acima do limite sem confirmação", action.risk));
        }
        if let Some(p) = action.permissions.iter().find(|p| !self.auto_allow.contains(p)) {
            return Decision::Confirm(format!("a permissão \"{}\" exige confirmação", p.label_pt()));
        }
        Decision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(perms: &[Permission], risk: Risk, reversible: bool) -> Action {
        Action { description: "teste".into(), permissions: perms.to_vec(), risk, reversible }
    }

    #[test]
    fn reading_is_allowed_by_default() {
        let p = Policy::default();
        assert_eq!(p.decide(&action(&[Permission::Read], Risk::None, true)), Decision::Allow);
    }

    #[test]
    fn writing_needs_confirmation_by_default() {
        let p = Policy::default();
        assert!(matches!(p.decide(&action(&[Permission::Write], Risk::Low, true)), Decision::Confirm(_)));
    }

    #[test]
    fn irreversible_always_confirms_even_if_permission_is_auto() {
        let p = Policy { auto_allow: vec![Permission::Read, Permission::Delete], ..Policy::default() };
        assert!(matches!(p.decide(&action(&[Permission::Delete], Risk::Low, false)), Decision::Confirm(_)));
    }

    #[test]
    fn system_is_denied_by_default() {
        let p = Policy::default();
        assert!(matches!(p.decide(&action(&[Permission::System], Risk::High, true)), Decision::Deny(_)));
    }

    #[test]
    fn high_risk_confirms_even_with_auto_permissions() {
        let p = Policy { auto_allow: vec![Permission::Network], ..Policy::default() };
        assert!(matches!(p.decide(&action(&[Permission::Network], Risk::High, true)), Decision::Confirm(_)));
    }
}
