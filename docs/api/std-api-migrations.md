# SQLite migrations example

O exemplo `examples/api/09_migrations.spectra` valida a leitura do schema final
e dos dados seed através da API SQLite file-backed.

O ciclo completo é certificado pelo validador independente, que usa o CLI real
e o framework de R-2503:

```powershell
python scripts\validate_r2514_migrations_example.py `
  --binary target\debug\spectralang.exe `
  --fixture tests\validation\202_migrations_multi_version.spectra `
  --database target\r2514-migrations-example\validation.sqlite `
  --migrations-dir tests\fixtures\r2514\migrations `
  --report target\r2514-migrations-example\report.json
```

São verificadas três versões, rollback da versão seed, reaplicação,
idempotência, checksums, drift, falha transacional e execução concorrente.
Não existe uma host call de migrations na linguagem nesta task; a fixture e o
validador possuem responsabilidades explícitas e separadas.
