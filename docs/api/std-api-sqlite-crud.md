# REST + SQLite CRUD

O exemplo `examples/api/06_rest_sqlite_crud.spectra` demonstra a superfície de
SQLite file-backed, migrations e o lifecycle do servidor. Ele não transforma
handlers estáticos em closures dinâmicas.

A prova dos endpoints CRUD dinâmicos está em
`packages/spectra-api/tests/rest_sqlite_crud.rs`. Esse harness Rust inicia o
`HttpServer` em TCP real, reutiliza `spectra-db`, aplica as migrations de
`tests/fixtures/r2511/migrations` e executa `GET`, `POST`, `PUT` e `DELETE`
contra o banco real usando queries parametrizadas.

Validação independente:

```powershell
python scripts\validate_r2511_rest_sqlite.py `
  --binary target\debug\spectralang.exe `
  --fixture tests\validation\201_rest_sqlite_crud.spectra `
  --database target\r2511-rest-sqlite\validation.sqlite `
  --migrations-dir tests\fixtures\r2511\migrations `
  --report target\r2511-rest-sqlite\report.json
```

O relatório separa explicitamente a fixture Spectra do harness HTTP dinâmico.
PostgreSQL, Redis e uma API genérica de callbacks Spectra permanecem fora do
escopo de R-2511.
