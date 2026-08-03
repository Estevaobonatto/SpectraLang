# `spectra.api.db.redis`

R-2507 fornece um driver Redis real para Redis 7, usando conexão externa,
health check `PING`, pool compartilhado e operações assíncronas fora do reactor.

A superfície disponível é `open`, `close`, `get`, `set`, `delete`, `expire`,
`incr` e `exists`. Pub/sub existe no contrato Rust de `spectra-db`; não há
host call de stream até que o protocolo de handles assíncronos da linguagem
possa representar notificações sem uma API incompleta.

A task permanece `in_progress` até a lane Redis 7 produzir o relatório
independente `passed`. Ausência de Redis local gera apenas
`skipped_environment` e não é evidência de produção.

Senhas, URLs completas, chaves e valores não são exportados pelo tracing por
padrão. O backend Redis é o consumidor previsto para R-2417 e R-2513.
