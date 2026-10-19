#!/usr/bin/env bash
# Prepara o repositório do DotCrow no GitHub:
#   1. cria o repositório PRIVADO (se ainda não existir) e envia o código;
#   2. configura descrição, tópicos, issues e discussões;
#   3. cria os rótulos, os marcos das fases e as primeiras issues.
#
# Pode rodar mais de uma vez: o que já existe é mantido.
#
# Uso (na raiz do repositório):
#   ./scripts/github/bootstrap.sh                  # usa napoleaomp4/dotcrow
#   ./scripts/github/bootstrap.sh usuario repo     # outro dono ou nome
#
# Requisitos: git e GitHub CLI (https://cli.github.com) com login feito (`gh auth login`).
# No Windows, rode no Git Bash.

set -euo pipefail

OWNER="${1:-napoleaomp4}"
REPO="${2:-dotcrow}"
FULL="$OWNER/$REPO"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
cd "$ROOT"

info() { printf '\n\033[1m▸ %s\033[0m\n' "$*"; }

command -v gh >/dev/null || { echo "Instale o GitHub CLI: https://cli.github.com"; exit 1; }
gh auth status >/dev/null 2>&1 || gh auth login

# 1. Repositório privado ------------------------------------------------------
info "Repositório $FULL"
if gh repo view "$FULL" >/dev/null 2>&1; then
  visibility="$(gh repo view "$FULL" --json visibility -q .visibility)"
  echo "Já existe (visibilidade: $visibility)."
  if [ "$visibility" != "PRIVATE" ]; then
    echo "ATENÇÃO: o repositório não está privado. Este script não muda a visibilidade de repositórios existentes."
  fi
  git remote get-url origin >/dev/null 2>&1 || git remote add origin "https://github.com/$FULL.git"
  git push -u origin main
else
  gh repo create "$FULL" --private --source=. --remote=origin --push \
    --description "Agentes open source que trabalham no seu computador. Várias LLMs com troca automática, chaves invisíveis e criptografia pós-quântica." \
    --homepage "https://dotcrow.si"
fi

gh repo edit "$FULL" \
  --enable-issues --enable-discussions --enable-wiki=false --enable-projects \
  --enable-squash-merge --enable-merge-commit=false --enable-rebase-merge \
  --delete-branch-on-merge \
  --add-topic ai-agents --add-topic llm --add-topic rust --add-topic automation \
  --add-topic post-quantum-cryptography --add-topic multi-agent --add-topic open-source --add-topic brasil

# 2. Rótulos ------------------------------------------------------------------
info "Rótulos"
while IFS=$'\t' read -r name color desc; do
  [ -z "$name" ] && continue
  gh label create "$name" --color "$color" --description "$desc" --force -R "$FULL" >/dev/null
  echo "  $name"
done < "$HERE/labels.tsv"

# 3. Marcos (fases) -----------------------------------------------------------
info "Marcos"
existing_ms="$(gh api "repos/$FULL/milestones?state=all&per_page=100" -q '.[].title')"
while IFS=$'\t' read -r title desc; do
  [ -z "$title" ] && continue
  if grep -Fxq "$title" <<<"$existing_ms"; then
    echo "  $title (já existe)"
  else
    gh api "repos/$FULL/milestones" -f title="$title" -f description="$desc" >/dev/null
    echo "  $title"
  fi
done < "$HERE/milestones.tsv"

# 4. Primeiras issues ---------------------------------------------------------
info "Issues"
existing_issues="$(gh issue list -R "$FULL" --state all --limit 500 --json title -q '.[].title')"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
for f in "$HERE"/issues/*.md; do
  title="$(sed -n 's/^title: //p' "$f" | head -1)"
  labels="$(sed -n 's/^labels: //p' "$f" | head -1)"
  milestone="$(sed -n 's/^milestone: //p' "$f" | head -1)"
  sed '1,/^---$/d' "$f" > "$tmp"
  if grep -Fxq "$title" <<<"$existing_issues"; then
    echo "  já existe: $title"
    continue
  fi
  args=(-R "$FULL" --title "$title" --body-file "$tmp" --label "$labels")
  [ -n "$milestone" ] && args+=(--milestone "$milestone")
  gh issue create "${args[@]}" >/dev/null
  echo "  criada: $title"
done

info "Pronto!"
cat <<MSG
Repositório: https://github.com/$FULL

Falta fazer à mão (o GitHub não deixa por script):
  • Settings → General → Social preview: envie assets/brand/png/dotcrow-social-preview.png
  • Para chamar colaboradores enquanto o repositório é privado:
      gh api -X PUT repos/$FULL/collaborators/USUARIO -f permission=push
MSG
