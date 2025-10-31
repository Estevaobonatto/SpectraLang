# SpectraLang – Próximos Passos Imediatos

1. **Match exaustivo e refinamentos de enum**
   - ✅ Construtores de enum e pattern matching parcial entregues (ver testes `match_accepts_enum_variant_patterns`, `match_binds_enum_tuple_payload`, `match_struct_variant_binding`).
   - Implementar verificação de exaustividade/duplicidades em `match`, incluindo recomendações para `_` quando necessário.
   - Estender padrões para suportar guardas e alias nomeados, mantendo diagnósticos precisos.

2. **Aliases e import seletivo**
   - Introduzir suporte sintático a `import lib.types as types` e `import lib.types::{Point, Flag}`.
   - Ajustar o resolvedor semântico para materializar aliases e conjuntos selecionados dentro do escopo de módulo, com conflitos devidamente relatados.
   - Atualizar o mecanismo de exportação para propagar metadados de structs/enums junto com spans.

3. **Checagem de structs entre módulos**
   - Propagar a lista de campos e tipos associados durante `collect_exports` para que consumidores validem acessos a partir de módulos externos.
   - Incluir erros direcionados para campos inexistentes, tipos incompatíveis e diferenças de visibilidade.
   - Cobrir o fluxo com testes multi-módulo que envolvam reexports.

4. **Anotações de tipo avançadas**
   - Aceitar arrays tipados (`let values: i32[] = [...]`) e garantir que o analisador respeite a anotação durante inferência.
   - Permitir inicialização de constantes/variáveis com tipos importados, garantindo que spans apontem para o símbolo correto.
   - Preparar os alicerces para futuros generics adicionando ganchos no resolvedor de tipos.

5. **Governança e backlog**
   - Registrar workshop de alinhamento com stakeholders e consolidar premissas de escopo/prazo em documentação compartilhada.
   - Revisar o backlog (epics e stories) para refletir o progresso recente e reordenar prioridades conforme riscos identificados.
   - Atualizar ADRs ou registrar novos conforme decisões surgirem durante as próximas iterações.
