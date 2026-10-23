# Estado verificável do MachiavelliDB

O workspace contém os 25 crates da arquitetura (incluindo invariants, adicionado
ao mapa original de 24). Isso não significa que todas as 18 SPECs estejam concluídas.
Há uma implementação operacional local e componentes experimentais com limites explícitos.

## Implementado e verificado

- Core, integradores simpléticos, Hamiltoniano, derivadas e mapa tangente.
- WAL próprio, validação transacional de nós, recuperação de cauda incompleta,
  snapshots imutáveis, backup/restauração com checksum e recusa de sobrescrita.
- Backend RocksDB opcional com as 12 column families; testes nativos no WSL/D:.
  Atualmente os dados de nós usam cf_continentia/cf_phase_space; as outras CFs
  estão reservadas, sem implicar que seus subsistemas já persistam nelas.
- Arena mmap com páginas CoW em forks, snapshots antigos e promoção por CAS.
- CFR+ extensivo, validação de perfect recall, chance, políticas compartilhadas
  por information set e melhores respostas que respeitam informação oculta.
  Jogos de referência: Kuhn, Leduc e ataque/defesa de duas rotas.
- Crenças bayesianas estáveis em log, Pareto, Kelly fracionário, VCG de resultados
  finitos e Brier; APIs de biblioteca com testes próprios.
- Quórum Ed25519 com n=3f+1 e 2f+1 assinaturas distintas, vinculado a digest,
  sequência, época e validade. É software BFT; não hardware-backed MinBFT.
- Diques imutáveis com TTL/topologia; planejador de isolamento bidirecional e decoys
  com caminho explícito para o operador. São planos/visões, não ações externas.
- Fortuna Pareto reproduzível e Centauro com rollouts mmap isolados e candidatos
  a diques. O modelo atual é harmônico de referência, sem MCTS.
- PQL PEG com os 11 verbos e validação de orçamentos. SCOUT, WARGAME e PRINCE KUHN
  executam na API; mutações táticas retornam conflito enquanto faltarem políticas
  e confirmação administrativa real.
- REST e gRPC nativos na mesma porta, token obrigatório, duas consultas PQL
  concorrentes, limites de entrada e paginação REST por UUID/sequence.
- Lo Stato WebGL local em /observatory, com posição/momento e risco por nó.
  Sintaxe JavaScript verificada; renderização, cobertura completa e 60fps não qualificados.
- Cliente gRPC Heraclitus Append com autenticação e idempotência, testado em mock local.
  Append comum não é recibo Merkle nem AdminIntent.
- Fonte CUDA de dois kernels e fallback CPU determinístico. Runtime/launch CUDA ainda
  não implementados/qualificados; o código não seleciona silenciosamente uma GPU fictícia.
- Lean 4.15: nove propriedades principais, mais lemas auxiliares, sem provas admitidas:
  identidades simpléticas, preservação por shears/composição, CFR+ não negativo e
  verdade no leilão de segundo preço. Modelo inteiro exato, sem refinamento f64.

## Qualificação medida

| Gate | Resultado | Limite da evidência |
|---|---|---|
| M1 | 1.000.000 passos Verlet FPU n64; erro <1e-9 | modelo e passo de referência |
| M2 Kuhn | gap 0.000012641; 20.000 iterações | resultado numérico local |
| M2 Leduc | gap 0.000001834; 85.362 iterações em orçamento de 30s | resultado numérico local |
| M2 ataque/defesa | gap <1e-3 no teste de duas rotas | modelo pequeno explícito |
| M3 | 128 MB / 1M metadados; mediana 22,9 µs, p99 168,7 µs | não atende limite em todas as amostras |
| M4–M5 | testes de planos e isolamento de simulações | sem qualificação de disparo/produção |
| M6 | não qualificado | falta contrato remoto para AdminIntent externo e recibo Merkle |

## Trabalho que ainda impede chamar produção completa

1. Persistir índices de arestas, regret/infosets, ledger e diques nas respectivas CFs;
   integrar arena e armazenamento sem copiar o mapa inteiro a cada commit.
2. Integrar quórum/configuração administrativa persistida com mutações reais,
   transferência de créditos, buffers e atomicidade do protocolo BETRAY.
3. Confirmar/implementar operação remota de SPEC-0089 para ações do MachiavelliDB.
   O Heraclitus possui execute_admin interno, mas o contrato remoto inspecionado
   não expõe uma operação para autorizar nossos efeitos externos.
4. MCTS, regret-based pruning, Deep CFR, CE multijogador e filtro de partículas
   simplético; a crença bayesiana e CFR extensivo atuais não substituem esses requisitos.
5. Lançamento/integração CUDA e qualificação em hardware, HSM/MinBFT e gates de cauda.
6. Observabilidade, gestão de chaves/roles, testes físicos de power-loss,
   performance/recuperação em escala e provas de refinamento Rust/f64.

Não se iniciou Heraclitus no Windows nem se alteraram os serviços Nietzsche/EVA.
Nenhum teste de bridge gravou no Heraclitus real. Os testes encerram seus próprios servidores.

## Formatos e limites

O WAL próprio usa frames MCV1 com cabeçalho de 80 bytes (magic, tamanho, sequência,
BLAKE3 do cabeçalho, BLAKE3 do envelope/payload); máximo 16 MiB por transação.
Somente caudas incompletas são truncadas. Corrupção completa falha sem reparo automático.
Erro ambíguo de escrita invalida o writer; reabra e confira antes de repetir.

RocksDB e WAL próprio usam pastas distintas, sem migração implícita. Abrir uma pasta
de outro backend é recusado. Backup é snapshot portátil de nós; não preserva histórico
de transações nem configurações externas. Um backup/restauro incompleto fica no disco
para inspeção, sem marcador de conclusão.

As limitações matemáticas do manifesto também importam: mínimo local não prova bacia
global/irreversibilidade; Kelly não garante risco de ruína zero no mundo real; numa
variedade simplética pura só há Casimirs constantes. O core usa extensão de Poisson
com coordenadas constitucionais. Provas em inteiros não certificam floats.

## Reprodução

~~~powershell
Set-Location D:\DEV\MachiavelliDB
.\scripts\cargo.ps1 fmt --all '--' --check
.\scripts\cargo.ps1 test --workspace --offline
.\scripts\cargo.ps1 clippy --workspace --all-targets --offline '--' -D warnings
.\scripts\cargo.ps1 build --workspace --offline
.\scripts\lean.ps1 build
.\scripts\smoke-api.ps1
.\scripts\cargo.ps1 run -p machiavelli-cfr --example qualification --offline
.\scripts\cargo.ps1 run -p machiavelli-cow-arena --example qualification --offline
~~~

RocksDB precisa de compilador C++/libclang. Foi compilado e testado no Ubuntu-24.04
em D:\WSL, com CARGO_TARGET_DIR=/home/junior/cargo-target/machiavelli e
LIBCLANG_PATH=/home/junior/cargo-target/machiavelli/clang.
Use scripts/rocks.ps1 para repetir os testes nativos.
