# 0002. Criptografia híbrida pós-quântica

- **Status:** aceita
- **Data:** 2026-10-19

## Contexto

Queremos que o DotCrow seja resistente a computadores quânticos, inclusive contra quem grava o tráfego hoje para decifrar no futuro. Os algoritmos pós-quânticos padronizados pelo NIST (ML-KEM, FIPS 203; ML-DSA, FIPS 204) são recentes, e implementações novas podem ter falhas.

## Decisão

Usar sempre esquemas **híbridos**:

- acordo de chave: ML-KEM-768 **e** X25519, combinados com HKDF-SHA256 amarrando o texto cifrado do KEM e as chaves públicas (no espírito do combinador X-Wing);
- cifra: XChaCha20-Poly1305;
- assinaturas (planejado): Ed25519 **e** ML-DSA-65, ambas obrigatórias na verificação;
- transporte (planejado): TLS 1.3 com o grupo híbrido X25519MLKEM768.

Só usamos implementações de crates reconhecidos (RustCrypto, dalek).

## Alternativas consideradas

- **Só ML-KEM:** perde a proteção se surgir um ataque clássico contra a implementação ou o esquema.
- **Só X25519:** não resiste a computadores quânticos.
- **X-Wing padronizado:** é o objetivo; adotaremos quando o padrão e a implementação em Rust estiverem estáveis, com versão nova do formato do envelope.

## Consequências

- Envelopes ficam maiores (cerca de 1,1 KB de cabeçalho por causa do ML-KEM-768). Aceitável para o nosso uso.
- O formato tem byte de versão, permitindo migrar de esquema sem quebrar dados antigos.
- Precisamos de auditoria externa antes da 1.0.
