<p align="center">
  <img src="img/machiavelli-logo.jpg" alt="MachiavelliDB" width="500" />
</p>

<h1 align="center">MachiavelliDB</h1>

<p align="center"><strong>Adversarial Strategy, Counterfactual Simulation & Phase-Space Decision Engine</strong></p>

<p align="center">
  O terceiro pilar da Trindade Cognitiva: onde a intuição plástica do <strong>NietzscheDB</strong> e a lei imutável do <strong>HeraclitusDB</strong> encontram o cálculo estratégico sob neblina de guerra (<em>Fog of War</em>).
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-1.0.0--alpha-8B0000" alt="version 1.0.0-alpha" />
  <img src="https://img.shields.io/badge/core-Rust%202024%20(no__gc)-071D41" alt="Rust 2024" />
  <img src="https://img.shields.io/badge/geometry-Symplectic%20%2B%20Hamiltonian-4B0082" alt="Symplectic Geometry" />
  <img src="https://img.shields.io/badge/solver-CFR%2B%20%26%20Nash%20Equilibrium-B8860B" alt="CFR+ Solver" />
  <img src="https://img.shields.io/badge/allocation-VCG%20Zero--Sum%20Auction-168821" alt="VCG Auction" />
  <img src="https://img.shields.io/badge/license-GPL--2.0--only-0C326F" alt="GPL-2.0-only" />
</p>

> *"Julgo poder ser verdade que a Fortuna seja árbitra de metade das nossas ações, mas que, ainda assim, ela nos deixe governar a outra metade, ou quase isso. E comparo-a a um desses rios impetuosos que, quando se enfurecem, alagam as planícies, destroem as árvores e os edifícios... Mas isso não impede que os homens, nos tempos calmos, possam tomar providências com diques e barragens."*  
> — **Nicolau Maquiavel**, *O Príncipe*, Capítulo XXV

---

## 1. Manifesto: Por que o Terceiro Irmão Existe?

A infraestrutura moderna de dados sofre de uma cegueira temporal tripla. Bancos de dados tradicionais (PostgreSQL, MongoDB, Milvus) são **cadáveres passivos**: guardam apenas fotografias inertes do presente.

Para superar essa passividade, construímos dois extremos filosóficos e matemáticos em Rust:
1. **NietzscheDB (O Córtex Dionisíaco — Passado/Presente Plástico):** Opera sobre variedades riemannianas curvas (Poincaré, Klein, Riemann, Minkowski). Ele sente, associa, esquece por decaimento termodinâmico e consolida abstrações durante o sono. Responde à pergunta: ***"O que isto significa e como eu evoluo?"***
2. **HeraclitusDB (O Escrivão Apolíneo — Passado Incorruptível):** Opera sobre o log canônico *append-only* (`HRKL`), árvores de Merkle BLAKE3, carimbos do tempo RFC 3161 e *Durable Intent* (`SPEC-0089`). Ele governa, audita e prova. Responde à pergunta: ***"O que exatamente aconteceu e como eu provo isso perante um juiz?"***

Faltava o elo que olha para o **Futuro Hostil**.

Um agente autônomo em produção crítica (defesa cibernética, controle de malha energética, resposta a fraudes coordenadas, infraestrutura estatal sob ataque ou enxames autônomos) não vive num jardim pacífico. Ele vive num **jogo de informação imperfeita contra adversários ativos e contra o acaso implacável (*Fortuna*)**.

O **MachiavelliDB** nasce para responder à terceira e mais perigosa pergunta:

> ***"Diante de um ambiente hostil, recursos finitos e informação incompleta, qual é o próximo movimento que maximiza a sobrevivência e a dominância estratégica antes que o adversário aja?"***

### A Trindade Completa em Ação

| Dimensão | NietzscheDB | HeraclitusDB | MachiavelliDB |
|---|---|---|---|
| **Filósofo Guia** | Friedrich Nietzsche (*Vontade de Potência*) | Heráclito de Éfeso (*Logos & Fluxo*) | Nicolau Maquiavel (*Virtù vs. Fortuna*) |
| **Horizonte Temporal** | Memória Viva (Passado $\leftrightarrow$ Presente) | História Canônica (Passado Imutável) | Futuro Contrafactual (Presente $\to$ $t + \Delta t$) |
| **Geometria Nativa** | Hiperbólica / Riemanniana ($\mathbb{B}^n, \mathbb{K}^n, S^n, M^{n,1}$) | Ordem Total Linear + DAG Causal (`LSN` + `HLC`) | Variedade Simplética & Espaço de Fase ($T^*Q, \omega$) |
| **Unidade Atômica** | `NodeMeta` + `PoincareVector` (Memória) | `CanonicalRecord` (Fato Auditável) | `ContinentiaNode` (Estado em Disputa + Momento) |
| **Motor de Fundo** | `EVA` (28 fases, Sono REM, L-Systems) | `Sentinel` (L0–L6) + Consenso `Raft` | `CentaurEngine` (CFR+, War-Gaming CoW, Diques) |
| **Linguagem Nativa** | `AQL` / `NQL` (Intenção Cognitiva) | `GQL` / `SQL AS OF` (Auditoria e Prova) | `PQL` — *Prince Query Language* (Ação Tática) |
| **Quando a falha ocorre** | Cicatriza com `PhantomNodes` (*Shatter*) | Recusa a ação (*Fail-Closed*) e gera prova | Executa `BETRAY` no ramo fraco e aciona o Dique pré-calculado |

---

## 2. Fundamentos Matemáticos: O Espaço de Fase da *Virtù*

Bancos de dados comuns armazenam coordenadas estáticas $q$. Mas saber **onde** um sistema está não diz **para onde ele está sendo arremessado**. O MachiavelliDB modela cada estado do domínio como um ponto em uma **Variedade Simplética** $2n$-dimensional $(\mathcal{M}, \omega)$, correspondente ao feixe cotangente $T^*Q$ do espaço de configuração.

### 2.1 Coordenadas Generalizadas ($q$) e Momento Estratégico ($p$)

Cada entidade ou subgrafo armazenado no MachiavelliDB possui um vetor de estado conjugado $z = (q, p) \in \mathbb{R}^{2n}$:
* **$q \in \mathbb{R}^n$ (Posição / Estado Atual):** A configuração observável do recurso, ativo, rota de rede ou hipótese (pode ser importada por projeção logarítmica $\log_0(x)$ diretamente do disco de Poincaré do NietzscheDB).
* **$p \in \mathbb{R}^n$ (Momento / Inércia Tática):** A taxa de variação ponderada pela "massa estratégica" do estado — para onde as forças atuais (ataques, consumo de recursos, degradação de latência, pressão adversarial) estão empurrando aquele nó.

A estrutura simplética fundamental é definida pela 2-forma diferencial fechada e não-degenerada:

$$\omega = \sum_{i=1}^{n} dq_i \wedge dp_i, \quad d\omega = 0$$

Em representação matricial para kernels SIMD/GPU, usamos a matriz simplética canônica $J$:

$$J = \begin{pmatrix} 0 & I_n \\ -I_n & 0 \end{pmatrix}, \quad J^2 = -I_{2n}, \quad J^T J = I_{2n}$$

### 2.2 O Hamiltoniano Tático $\mathcal{H}(q, p)$: *Virtù* menos *Fortuna*

A evolução temporal do banco de dados é governada por uma função energia mestre chamada **Hamiltoniano Estratégico** $\mathcal{H}: \mathcal{M} \to \mathbb{R}$, que decompõe a cinética da ação (*Virtù*) e o potencial de risco exógeno (*Fortuna*):

$$\mathcal{H}(q, p, t) = \underbrace{\frac{1}{2} p^T M^{-1}(q) p}_{\text{Cinética de Execução (Virtù)}} + \underbrace{V_{\text{Fortuna}}(q, t) + \Phi_{\text{Adversarial}}(q, \sigma_{-i})}_{\text{Potencial de Risco e Atrito Hostil}}$$

As trajetórias futuras de qualquer estado no banco não são "adivinhadas" por heurísticas cegas; elas fluem segundo as **Equações de Hamilton** preservando a estrutura simplética:

$$\frac{dq_i}{dt} = \frac{\partial \mathcal{H}}{\partial p_i}, \quad \frac{dp_i}{dt} = -\frac{\partial \mathcal{H}}{\partial q_i} + F_{\text{control}}(u)$$

Onde o colchete de Poisson $\{f, g\}$ mede instantaneamente se duas políticas ou subgrafos interferem entre si:

$$\{f, g\} = \sum_{i=1}^{n} \left( \frac{\partial f}{\partial q_i}\frac{\partial g}{\partial p_i} - \frac{\partial f}{\partial p_i}\frac{\partial g}{\partial q_i} \right)$$

Se $\{f, \mathcal{H}\} = 0$, a propriedade $f$ é uma **Invariante de Sobrevivência** (uma constante de movimento que nem o caos externo consegue destruir).

### 2.3 Teorema de Liouville e a Conservação da Incerteza

Pelo **Teorema de Liouville**, o fluxo hamiltoniano $\Phi_t$ preserva o volume no espaço de fase:

$$\text{Vol}(\Phi_t(U)) = \int_{\Phi_t(U)} \omega^n = \int_{U} \omega^n = \text{constante}$$

**Tradução para Engenharia de Sistemas:** No MachiavelliDB, você nunca "elimina" o risco gratuitamente. Se você aperta o controle e comprime a variância de posição $\Delta q$ numa rota crítica (ex.: travando recursos para um serviço prioritário), o volume de momento/pressão $\Delta p$ explode em outra dimensão. O banco calcula essa compressão em tempo real usando **Integradores Simpléticos de Störmer-Verlet / Yoshida de 4ª ordem**, que não sofrem de deriva energética secular (*energy drift*) mesmo após milhões de passos de simulação.

### 2.4 Minimização de Arrependimento Contrafactual (`CFR+`) sob Neblina de Guerra

Diferente do xadrez (informação perfeita), o mundo real é um jogo de **informação imperfeita** (*Fog of War*): você não vê as cartas do atacante nem sabe qual servidor vai queimar. 

O MachiavelliDB agrupa estados indistinguíveis pelo observador em **Conjuntos de Informação (*Information Sets*)** $I \in \mathcal{I}$. Para cada ação $a \in A(I)$ na iteração $T$, o motor calcula a utilidade contrafactual $v_i(\sigma, I)$ e acumula o arrependimento instantâneo truncado (`CFR+`, Tammelin 2014):

$$R^{T,+}(I, a) = \max\left(0, \; R^{T-1,+}(I, a) + v_i(\sigma^T_{I \to a}, I) - v_i(\sigma^T, I)\right)$$

A estratégia do próximo *tick* é atualizada via *Regret Matching+*:

$$\sigma^{T+1}(I)(a) = \begin{cases} \frac{R^{T,+}(I, a)}{\sum_{a' \in A(I)} R^{T,+}(I, a')} & \text{se } \sum_{a'} R^{T,+}(I, a') > 0 \\ \frac{1}{|A(I)|} & \text{caso contrário} \end{cases}$$

O perfil médio de estratégias converge com garantia matemática ($O(1/\sqrt{T})$) para um **Equilíbrio de Nash $\epsilon$-aproximado**, gerando **Diques (*Dikes*)**: planos de resposta pré-computados e gravados em memória *lock-free* para execução em microssegundos quando a *Fortuna* atacar.

---

## 3. Arquitetura do Workspace: Os 24 Crates em Rust

O MachiavelliDB segue a disciplina de ferro da família: Rust puro, zero *Garbage Collector*, *zero-copy deserialization* (`rkyv`), separação entre metadados táticos leves e tensores de payoff pesados, e isolamento em 24 *crates* organizados em 6 camadas estratégicas:

~~~text
                        +---------------------------------------+
                        |       PQL / gRPC / Trinitas Bus       |
                        |   (machiavelli-pql, machiavelli-api)  |
                        +-------------------+-------------------+
                                            |
              +-----------------------------+-----------------------------+
              |                                                           |
              v                                                           v
+---------------------------+                               +---------------------------+
|      THE CENTAUR          |                               |    VIRTÙ RESOURCE KERNEL  |
|   (Adversarial Daemon)    |                               |     (Zero-Sum Economy)    |
|  - machiavelli-centaur    |                               |  - machiavelli-vcg        |
|  - machiavelli-cfr        |                               |  - machiavelli-betrayal   |
|  - machiavelli-fortuna    |                               |  - machiavelli-fortify    |
|  - machiavelli-dikes      |                               |  - machiavelli-pareto     |
+-------------+-------------+                               +-------------+-------------+
              |                                                           |
              +-----------------------------+-----------------------------+
                                            |
                                            v
                        +---------------------------------------+
                        |     SYMPLECTIC PHASE-SPACE ENGINE     |
                        |  - machiavelli-symplectic             |
                        |  - machiavelli-hamiltonian            |
                        |  - machiavelli-nash                   |
                        |  - machiavelli-fog (InfoSets)         |
                        +-------------------+-------------------+
                                            |
                                            v
                        +---------------------------------------+
                        |   STORAGE, CoW ARENA & HARDWARE ACCEL |
                        |  - machiavelli-core                   |
                        |  - machiavelli-storage (RocksDB 12CF) |
                        |  - machiavelli-cow-arena (mmap fork)  |
                        |  - machiavelli-wal (Intent + Action)  |
                        |  - machiavelli-gpu (CUDA CFR/Verlet)  |
                        +-------------------+-------------------+
                                            |
                      +---------------------+---------------------+
                      |                                           |
                      v                                           v
         +-------------------------+                 +-------------------------+
         |   nietzsche-bridge      |                 |   heraclitus-bridge     |
         | (Poincaré <-> PhaseSpc) |                 | (DurableIntent / HRKL)  |
         +-------------------------+                 +-------------------------+
~~~

### Mapa Completo dos 24 Crates

| Camada | Crate | Responsabilidade Técnica |
|---|---|---|
| **I. Fundação Simplética (4)** | `machiavelli-core` | Tipos primitivos: `ContinentiaNode` (128 bytes), `PhaseVector` $(q,p)$, `StrategicEdge`, `VirtuScore`, `FortunaRisk`. |
| | `machiavelli-symplectic` | Forma simplética $\omega$, matriz $J$, colchetes de Poisson $\{f,g\}$, integradores de Störmer-Verlet e Yoshida 4ª ordem em `f64` + AVX-512. |
| | `machiavelli-hamiltonian` | Cálculo de energia cinética (*Virtù*), potencial de risco (*Fortuna*) e deteção de invariantes de Liouville. |
| | `machiavelli-proto` | Contratos Protocol Buffers v3 e definições gRPC de baixa latência. |
| **II. Armazenamento & CoW (4)** | `machiavelli-storage` | Persistência sobre RocksDB com 12 *Column Families* isolando metadados táticos de tensores de jogo. |
| | `machiavelli-cow-arena` | Arena de memória mapeada (`mmap`) com *Copy-on-Write* em nível de página para clonar o banco em $< 50\,\mu\text{s}$ durante simulações de guerra. |
| | `machiavelli-wal` | Write-Ahead Log binário v1 com CRC-32C e suporte a *Speculative Branching* (ramos descartáveis sem poluir o estado real). |
| | `machiavelli-dikes` | Índice ultra-rápido em RAM (`ArcSwap` + `rkyv`) que guarda planos de contingência pré-computados (*Diques*) prontos para disparo em $O(1)$. |
| **III. Teoria dos Jogos & Neblina (4)** | `machiavelli-nash` | Matrizes de bimatriz, tensores polimatrículas, cálculo de Equilíbrio de Nash e Equilíbrio Correlacionado (CE). |
| | `machiavelli-cfr` | Motor `CFR+` (Counterfactual Regret Minimization Plus) e *Deep CFR* para jogos extensivos de informação imperfeita. |
| | `machiavelli-fog` | Gestão de *Information Sets* (*Fog of War*), inferência bayesiana de estados ocultos do adversário e deteção de *bluffs*/anomalias. |
| | `machiavelli-pareto` | Fusão de estados conflitantes por Fronteira de Dominância de Pareto e Critério de Kelly (alocação ótima sob risco de ruína). |
| **IV. Economia de Soma Zero (4)** | `machiavelli-vcg` | Leilão de Vickrey-Clarke-Groves para alocação competitiva de RAM, CPU e I/O entre subgrafos com base na utilidade preditiva. |
| | `machiavelli-betrayal` | Protocolo de corte seco (*Zero-Grace Betrayal*): amputação atômica de subgrafos, rotas ou agentes comprometidos sem cascata de falha. |
| | `machiavelli-fortify` | Isolamento criptográfico e consenso Bizantino local (MinBFT) para nós classificados como *Cidadelas* (ativos inegociáveis). |
| | `machiavelli-mandragola` | Subsistema de decepção ativa (*Honeypots* topológicos e nós *Decoy* que alimentam invasores ou *queries* hostis com vetores falsos). |
| **V. O Centauro & Linguagem (4)** | `machiavelli-centaur` | Daemon autônomo de *War-Gaming* contínuo (metade força bruta MCTS/CFR, metade heurística tática). |
| | `machiavelli-fortuna` | Gerador de caos adversarial: injeta choques de Cisne Negro, falhas bizantinas e *stress* topológico nas arenas CoW. |
| | `machiavelli-pql` | Parser PEG (`pest`), planejador estratégico e executor da **PQL (*Prince Query Language*)**. |
| | `machiavelli-gpu` | Kernels CUDA para rollouts paralelos de CFR+ e integração simplética massiva em GPU. |
| **VI. Pontes & Superfície (4)** | `machiavelli-trinitas` | Conector nativo gRPC/Zero-Copy para o **NietzscheDB** (importação de intuições) e **HeraclitusDB** (submissão ao `SPEC-0089` / `HRKL`). |
| | `machiavelli-api` | Servidor gRPC + REST e streaming de telemetria tática. |
| | `machiavelli-observatory` | Backend para o painel visual **Lo Stato** (renderização de retratos de fase, bacias de atração e fronteiras de Pareto a 60fps). |
| | `machiavelli-cli` | Console interativo para operação tática, injeção de choques e inspeção de Diques. |

---

## 4. Estruturas de Dados Nucleares e As 12 Column Families

Seguindo a lição de performance provada no NietzscheDB (onde separar `NodeMeta` do embedding gerou 15x de *speedup*), o `machiavelli-storage` divide cada arena em **12 Column Families no RocksDB**:

| CF | Chave | Valor | Propósito Tático |
|---|---|---|---|
| `cf_continentia` | `node_id (16B)` | `ContinentiaMeta` (128B) | Metadados leves de estado: *Virtù*, *Fortuna*, massa estratégica, flags de fortificação e traição. |
| `cf_phase_space` | `node_id (16B)` | `PhaseVector` (`rkyv`) | Coordenadas conjugadas $(q \in \mathbb{R}^n, p \in \mathbb{R}^n)$ em precisão `f64`. |
| `cf_payoff_tensors` | `edge_id (16B)` | `PayoffMatrix` (`rkyv`) | Matrizes de recompensa/risco bimatuais para cada interação estratégica entre nós. |
| `cf_adj_strategic` | `node_id (16B)` | `Vec<StrategicAdj>` | Lista de adjacência rápida (aliados, dependências críticas, vetores de ameaça e rotas de fuga). |
| `cf_infosets` | `infoset_hash (32B)`| `InformationSet` | Agrupamento de nós indistinguíveis sob *Fog of War* para o solver CFR+. |
| `cf_regret_tables` | `infoset_hash (32B)`| `RegretVector` (`rkyv`) | Tabelas de arrependimento acumulado $R^{T,+}(I, a)$ e estratégia média $\bar{\sigma}(I)$. |
| `cf_dikes` | `trigger_hash (32B)`| `CompiledDike` | Planos de contingência pré-calculados pelo Centauro para disparo instantâneo. |
| `cf_vcg_ledger` | `subgraph_id (16B)` | `VcgAccount` | Saldo de créditos de *Virtù*, lances (*bids*) de recursos e histórico de precisão preditiva. |
| `cf_deception` | `node_id (16B)` | `PhantomDecoy` | Coordenadas e payloads falsos servidos a observadores não confiáveis (*Mandragola*). |
| `cf_invariants` | `invariant_id (16B)`| `CasimirConstraint` | Regras de sobrevivência invioláveis ($\{f, \mathcal{H}\} = 0$) monitoradas a cada tick. |
| `cf_bft_quorum` | `node_id (16B)` | `BftSeal` | Assinaturas Ed25519 e votos de quórum para nós fortificados (*Cidadelas*). |
| `cf_trinitas_sync` | `lsn_or_uuid (16B)` | `TrinitasAnchor` | Ponteiros cruzados entre o UUID hiperbólico (NietzscheDB) e o LSN canônico (HeraclitusDB). |

### A Estrutura `ContinentiaMeta` (128 Bytes Alinhados em Cache Line)

```rust
#[repr(C, align(64))]
pub struct ContinentiaMeta {
    pub id: Uuid,                      // 16B - Identidade única do estado
    pub virtu: f32,                    // 4B  - [0.0, 1.0] Capacidade de execução e precisão histórica
    pub fortuna_exposure: f32,         // 4B  - [0.0, 1.0] Vulnerabilidade a choques exógenos
    pub strategic_mass: f32,           // 4B  - Inércia (resistência a mudanças bruscas de momento)
    pub hamiltonian_energy: f64,       // 8B  - H(q, p) instantâneo
    pub nash_reach_prob: f32,          // 4B  - Probabilidade de este estado ser alcançado no Equilíbrio de Nash
    pub cumulative_regret: f32,        // 4B  - Arrependimento contrafactual acumulado
    pub vcg_credit_balance: f64,       // 8B  - Moeda interna para disputar RAM/CPU no leilão VCG
    pub posture: StrategicPosture,     // 1B  - Expanding | Fortified | expendable | Decoy | Betrayed
    pub fog_level: u8,                 // 1B  - [0..255] Opacidade de informação (0 = visível, 255 = neblina total)
    pub alliance_cluster: u16,         // 2B  - ID da coalizão atual
    pub dike_count: u32,               // 4B  - Quantidade de diques (planos de contingência) anexados
    pub parent_nietzsche_id: Uuid,     // 16B - Origem conceitual no NietzscheDB (se houver)
    pub last_heraclitus_lsn: u64,      // 8B  - Último LSN auditado no HeraclitusDB
    pub created_at_hlc: u64,           // 8B  - Timestamp Hybrid Logical Clock
    pub betrayal_horizon_secs: u64,    // 8B  - Tempo limite antes de o nó ser considerado passivo descartável
    pub _reserved: [u8; 24],           // 24B - Padding exato para fechar 128 bytes (2 cache lines x86_64)
}
```

---

## 5. PQL — *Prince Query Language* (A Linguagem da Ação Estratégica)

O SQL seleciona linhas mortas. A AQL evoca memórias. O GQL audita o passado.
A **PQL** foi desenhada para comandar sistemas sob pressão, simular cenários adversariais e executar manobras cirúrgicas. Ela possui **11 verbos imperativos-estratégicos**:

| Verbo PQL | Intenção Maquiavélica | Operação Matemática / Física no Motor |
|---|---|---|
| `SCOUT` | Sondar o terreno sob neblina | Amostragem no espaço de fase $(q,p)$ com estimativa de entropia de *Information Set* sem alterar o estado observável. |
| `WARGAME` | Simular batalha de futuros | Clona a arena via `mmap` CoW e executa $N$ iterações de `CFR+` / Störmer-Verlet contra um perfil adversarial. |
| `FORTUNA` | Injetar o acaso / Cisne Negro | Perturba o potencial $V_{\text{Fortuna}}(q, t)$ com choques extremos (distribuição de Cauchy/Lévy) para medir fragilidade. |
| `DIKE` | Construir barragem preventiva | Compila a melhor resposta do *War-Game* num plano atômico `CompiledDike` atrelado a um predicado de disparo em $O(1)$. |
| `FORTIFY` | Transformar nó em Cidadela | Eleva a massa estratégica $M(q)$, congela mutações externas e passa a exigir quórum BFT + `DurableIntent` no HeraclitusDB. |
| `ALLY` | Formar coalizão de soma positiva | Cria aresta de cooperação com partilha de créditos VCG e acoplamento simplético de fase. |
| `BETRAY` | Amputar ramo passivo/comprometido | Executa corte seco (*zero-grace*): zera alocação VCG, isola adjacências em $< 10\,\mu\text{s}$ e drena créditos restantes para o núcleo. |
| `DECEIVE` | Plantar ilusão (*Mandragola*) | Cria projeção fantasma em `cf_deception` que devolve gradientes invertidos para consultas originadas fora da coalizão. |
| `CONQUER` | Absorver estado por dominância | Funde dois subgrafos na Fronteira de Pareto: o nó dominante absorve a *Virtù* e as conexões úteis do subordinado. |
| `TRIBUTE` | Cobrar imposto de performance | Executa rodada imediata do Leilão VCG, penalizando subgrafos com erro de previsão alto e premiando os precisos. |
| `PRINCE` | Invocar deliberação executiva | Retorna a ação ótima do Equilíbrio de Nash atual com recibo de arrependimento limitado ($\text{Regret} < \epsilon$). |

### Exemplos de Sintaxe PQL

```sql
-- 1. Simular um ataque coordenado à malha de autenticação e pré-compilar um Dique
WARGAME Scenario:"credential_stuffing_cascade" ON COLLECTION "infra_core"
    ADVERSARY PROFILE aggressive_adaptive
    HORIZON 120s ROLLOUTS 50000
    SIMULATE FORTUNA SHOCK 0.90 DISTRIBUTION levy
THEN DIKE "isolate_edge_gateway"
    WHEN hamiltonian_energy > 0.85 OR fortuna_exposure > 0.75
    ACTION BETRAY @compromised_routes AND FORTIFY @auth_citadel;

-- 2. Consultar a melhor decisão tática sob neblina de guerra com garantia para o HeraclitusDB
SCOUT Infoset:"fraud_ring_alpha" FOG_TOLERANCE 0.35
THEN PRINCE STRATEGY nash_equilibrium
    MAX_REGRET 0.005
    KELLY_FRACTION 0.5
    EMIT HERACLITUS_INTENT;
```

---

## 6. Catálogo Completo de Especificações (`SPEC-0001` a `SPEC-0018`)

Para construir o **MachiavelliDB** do zero até a qualificação de produção sem ambiguidades, toda a engenharia do projeto é dividida em **18 Especificações Formais (`SPEC.md`)**. Abaixo está a descrição completa do que cada especificação exige e define.

---

### `SPEC-0001` — Kernel Simplético e Espaço de Fase (`machiavelli-symplectic`)
* **Objetivo:** Definir a representação matemática exata do espaço de fase $(q, p) \in T^*Q$ e garantir que nenhuma operação de evolução viole a geometria simplética.
* **Requisitos Obrigatórios:**
  1. Vetores `PhaseVector` devem ter dimensão par $2n$ ($n$ coordenadas de configuração $q_i$ e $n$ coordenadas de momento $p_i$, tipicamente $n = 64$ ou $128$, totalizando $128$ ou $256$ floats `f64`).
  2. Implementação de integradores simpléticos explícitos de **Störmer-Verlet (2ª ordem)** e **Yoshida (4ª ordem)**. É **proibido** usar o método de Euler explícito ou Runge-Kutta clássico (RK4) não-simplético, pois eles introduzem deriva artificial na energia $\mathcal{H}$ ao longo do tempo.
  3. Operador de Colchete de Poisson `{f, g}` vetorizado em AVX-512 / NEON.
  4. **Invariante de Qualificação:** Após $10^5$ passos de integração contínua num sistema fechado de teste, o erro relativo do volume de Liouville e da energia hamiltoniana deve satisfazer $|\Delta \mathcal{H} / \mathcal{H}_0| < 10^{-9}$.

---

### `SPEC-0002` — Funcional Hamiltoniano: *Virtù*, *Fortuna* e Massa Estratégica (`machiavelli-hamiltonian`)
* **Objetivo:** Formalizar como o banco quantifica o poder de execução interna (*Virtù*) versus a exposição ao caos externo (*Fortuna*).
* **Requisitos Obrigatórios:**
  1. Cálculo do tensor de massa estratégica diagonal $M(q) = \text{diag}(m_1, \dots, m_n)$, onde nós centrais/fortificados possuem maior inércia (exigem forças colossais para mudar de estado) e nós periféricos (*scouts*) possuem massa leve para manobra rápida.
  2. Campo potencial $V_{\text{Fortuna}}(q, t)$ alimentado por métricas de anomalia em tempo real (latência de rede, taxa de erros, divergência de entropia).
  3. Deteção automática de **Pontos de Sela e Bacias de Atração**: identificar quando um estado está prestes a cair num poço de potencial irreversível (falha catastrófica) antes de cruzar a separatriz.

---

### `SPEC-0003` — Armazenamento `Continentia` e Arena *Copy-on-Write* (`machiavelli-storage`, `machiavelli-cow-arena`)
* **Objetivo:** Permitir que o banco execute milhares de simulações destrutivas ("E se matarmos metade do cluster?") por segundo sem corromper nem bloquear as leituras do estado real.
* **Requisitos Obrigatórios:**
  1. Persistência base em RocksDB com as 12 *Column Families* definidas na Seção 4.
  2. **Arena `mmap` Copy-on-Write (CoW):** Os vetores de fase $(q,p)$ e matrizes de payoff ativas residem em segmentos mapeados em memória com páginas protegidas (`PROT_READ`). Quando o motor de *War-Gaming* inicia uma simulação, ele cria um descritor de arena clone em $O(1)$ ($< 50\,\mu\text{s}$) usando páginas virtuais CoW (`MAP_PRIVATE`).
  3. Apenas as páginas efetivamente mutadas pelo cenário contrafactual alocam RAM física (*dirty pages*), que são descartadas instantaneamente em $O(1)$ ao fim do *rollout* ou promovidas atomicamente via `ArcSwap` caso o cenário se torne realidade.

---

### `SPEC-0004` — Conjuntos de Informação e Neblina de Guerra (`machiavelli-fog`)
* **Objetivo:** Modelar o desconhecimento parcial do ambiente e a assimetria de informação contra agentes externos ou atacantes.
* **Requisitos Obrigatórios:**
  1. Um `InformationSet` $I$ agrupa múltiplos nós `Continentia` que produzem exatamente os mesmos sinais observáveis para o agente num dado instante, mas possuem estados ocultos distintos (ex.: "Pico de tráfego legítimo" vs. "Ataque DDoS camuflado").
  2. Manutenção de uma distribuição de crença bayesiana (*Belief State*) $b(s) \in \Delta(I)$ sobre os nós de cada conjunto de informação, atualizada via filtro de partículas simplético a cada novo evento ingerido.
  3. Proibição de *Clairvoyance Leak*: o planejador PQL nunca pode tomar decisões baseadas em variáveis ocultas de um nó individual se o `fog_level` exigir resolução no nível do `InformationSet`.

---

### `SPEC-0005` — Solver `CFR+` e Equilíbrio de Nash em Tempo Real (`machiavelli-cfr`, `machiavelli-nash`)
* **Objetivo:** Encontrar estratégias inexploráveis (*unexploitable strategies*) em grafos de decisão sob informação imperfeita.
* **Requisitos Obrigatórios:**
  1. Implementação nativa em Rust do algoritmo **CFR+ (Counterfactual Regret Minimization Plus)**:
     * Regrets negativos truncados imediatamente em zero ($R^{T,+} = \max(0, R^T)$).
     * Atualização sequencial em profundidade com poda alfa-beta baseada em arrependimento (*Regret-Based Pruning*).
     * Média ponderada linearmente das estratégias ($\bar{\sigma}^T = \frac{2}{T(T+1)} \sum_{t=1}^T t \sigma^t$).
  2. Suporte a jogos de soma zero (2 jogadores: Sistema vs. *Fortuna*/Atacante) via CFR+ e jogos gerais de múltiplos agentes via **Equilíbrio Correlacionado (Correlated Equilibrium)** resolvido por programação linear leve embutida.
  3. **Métrica de Exploitabilidade ($\epsilon$):** Todo plano gerado deve informar sua distância matemática para o Equilíbrio de Nash exato.

---

### `SPEC-0006` — O Motor Centauro: *War-Gaming* Autônomo Contínuo (`machiavelli-centaur`, `machiavelli-fortuna`)
* **Objetivo:** Como Maquiavel ensina que o Príncipe nunca deve tirar o pensamento do exercício da guerra (e ainda mais na paz do que na guerra), este daemon ataca o próprio banco continuamente em *background*.
* **Requisitos Obrigatórios:**
  1. Ciclo de batimento (*Centaur Tick*) operando sobre arenas CoW isoladas.
  2. **Módulo *Fortuna* (Red-Team Interno):** Gera perturbações estocásticas de cauda longa (distribuições de Pareto/Lévy — Cisnes Negros), simula queda de nós de consenso, corrupção de latência e esgotamento de recursos.
  3. **Módulo *Quíron* (Blue-Team Interno):** Roda MCTS acoplado ao CFR+ para descobrir qual sequência de comandos PQL (`BETRAY`, `FORTIFY`, `ALLY`) minimiza o dano máximo (*Minimax Regret*).
  4. Quando uma vulnerabilidade estrutural é descoberta pelo Centauro antes de acontecer em produção, ele sintetiza automaticamente um **Dique (`CompiledDike`)**.

---

### `SPEC-0007` — Diques de Maquiavel: Planos de Contingência Pré-Compilados (`machiavelli-dikes`)
* **Objetivo:** Garantir resposta em tempo sub-milissegundo ($< 100\,\mu\text{s}$) quando uma crise prevista pelo Centauro se materializa no mundo real.
* **Requisitos Obrigatórios:**
  1. Um `CompiledDike` contém:
     * `TriggerPredicate`: condição booleana rápida sobre variáveis de fase $(q, p)$ e energia $\mathcal{H}$.
     * `ActionBytecode`: sequência pré-compilada de mutações de estado e comandos de contenção.
     * `ExpectedSavedVirtu`: ganho estimado de sobrevivência validado em simulação.
     * `TTL & Decay`: diques obsoletos para topologias que já mudaram sofrem evicção automática.
  2. Na ingestão de qualquer telemetria, o vetor de estado passa por uma árvore de decisão SIMD de Diques ativos. Se o gatilho bater, a resposta já está pronta — zero tempo gasto pensando durante o incêndio.

---

### `SPEC-0008` — Economia VCG: Alocação de Recursos de Soma Zero (`machiavelli-vcg`)
* **Objetivo:** Substituir o agendamento ingênuo (*round-robin* / *fair-share*) por darwinismo econômico implacável baseado em verdade preditiva.
* **Requisitos Obrigatórios:**
  1. Cada subgrafo, agente ou *tenant* possui uma conta em `cf_vcg_ledger` com saldo de **Créditos de *Virtù***.
  2. A cada época de alocação, os subgrafos disputam cotas de RAM (tamanho de cache), threads de CPU e prioridade de GPU em um **Leilão de Vickrey-Clarke-Groves (VCG)**.
  3. **Propriedade *Strategy-Proof* (Incentivo à Verdade):** Pelo mecanismo VCG, cada participante paga apenas a "externalidade" (o dano que sua presença causa aos demais subgrafos). Mentir sobre a própria urgência ou importância resulta em perda matemática líquida de créditos.
  4. **Recompensa por Previsão (Scoring Estritamente Próprio de Brier):** Subgrafos cujas estimativas de futuro se confirmam ganham dividendos de *Virtù*; subgrafos que alucinam ou erram previsões têm seu saldo confiscado e sofrem rebaixamento para *Cold Storage*.

---

### `SPEC-0009` — Protocolo de Traição Cirúrgica e Amputação (`machiavelli-betrayal`)
* **Objetivo:** Implementar a máxima maquiavélica de que o mal deve ser feito todo de uma só vez (*"le ingiurie si debbono fare tutte insieme"*), evitando que um componente agonizante drene o sistema inteiro em cascata.
* **Requisitos Obrigatórios:**
  1. Operação atômica `BETRAY`: quando um nó ou subgrafo ultrapassa o limiar de passivo estratégico ($\mathcal{H}_{\text{custo}} > \theta_{\text{ruína}}$ e probabilidade de recuperação $< \epsilon$), o corte é instantâneo (*Zero-Grace*).
  2. Não há *drain* lento de fila nem espera por *timeouts* de rede: todas as arestas de entrada/saída recebem um *circuit-break* físico em memória, os buffers pendentes são descartados com recibo de amputação e os créditos VCG remanescentes são transferidos imediatamente para os nós sobreviventes da coalizão.
  3. Toda traição gera obrigatoriamente um registro de causa-raiz enviado à ponte do **HeraclitusDB** para auditoria pós-guerra.

---

### `SPEC-0010` — Cidadelas e Fortificação Bizantina (`machiavelli-fortify`)
* **Objetivo:** Proteger o núcleo inegociável da arquitetura contra corrupção, mesmo que a periferia inteira do grafo tenha sido traída ou invadida.
* **Requisitos Obrigatórios:**
  1. Nós marcados com `FORTIFY` entram no estado `StrategicPosture::Fortified` (*Cidadela*).
  2. Qualquer mutação de coordenadas $(q,p)$, alteração de matriz de payoff ou comando `BETRAY` contra uma Cidadela exige **Tolerância a Falhas Bizantinas (MinBFT)** com $2f + 1$ assinaturas Ed25519 verificadas em hardware/HSM.
  3. A massa estratégica $M(q)$ de uma Cidadela é multiplicada por um fator $\gamma_{\text{citadel}} \ge 100$, tornando-a imune a perturbações gravitacionais ou dinâmicas de nós periféricos.

---

### `SPEC-0011` — Operação *Mandragola*: Decepção Ativa e *Honeypots* Topológicos (`machiavelli-mandragola`)
* **Objetivo:** Batizado em homenagem à peça *A Mandrágora* de Maquiavel, este módulo garante que observadores hostis ou *queries* não confiáveis nunca vejam o mapa real do poder.
* **Requisitos Obrigatórios:**
  1. Capacidade de criar **Nós *Decoy* (Iscas)** e **Espelhos de Fase Falsos** em `cf_deception`.
  2. Quando uma consulta chega com credencial sob suspeita (sinalizada pelo *Sentinel* do HeraclitusDB) ou padrão de *scraping* adversarial, o motor não retorna erro `403 Forbidden` (o que alertaria o atacante). Em vez disso, serve uma **Variedade Simplética Espelhada**, onde os gradientes hamiltonianos apontam sutilmente para armadilhas estratégicas (becos sem saída computacionais).
  3. Toda interação de um agente externo com um nó *Decoy* alimenta imediatamente o solver `CFR+` com o perfil comportamental do invasor.

---

### `SPEC-0012` — Conquista de Pareto e Critério de Kelly (`machiavelli-pareto`)
* **Objetivo:** Definir como planos e estados concorrentes são fundidos sem a "democracia ingênua" de uma média aritmética.
* **Requisitos Obrigatórios:**
  1. Enquanto o NietzscheDB usa a Média de Fréchet na Esfera de Riemann para conciliar opostos harmonicamente, o MachiavelliDB usa a **Fronteira de Dominância de Pareto** ($\succ_{\text{Pareto}}$): um estado $A$ conquista o estado $B$ se $A$ é superior ou igual em todos os vetores de *Virtù* e estritamente menor em exposição à *Fortuna*.
  2. Para alocação de recursos sob incerteza estocástica, o dimensionamento da aposta segue o **Critério de Kelly Fracionário** ($f^* = \kappa \frac{bp - q}{b}$, com $\kappa \in [0.25, 0.5]$), garantindo matematicamente **Risco de Ruína Zero** ($\text{Prob}(\text{Virtù} \to 0) = 0$) no longo prazo.

---

### `SPEC-0013` — Compilador e Planejador da PQL (`machiavelli-pql`)
* **Objetivo:** Especificar a gramática formal PEG, a verificação de tipos estratégicos e o *lowering* da *Prince Query Language*.
* **Requisitos Obrigatórios:**
  1. Gramática PEG determinística para os 11 verbos (`SCOUT`, `WARGAME`, `FORTUNA`, `DIKE`, `FORTIFY`, `ALLY`, `BETRAY`, `DECEIVE`, `CONQUER`, `TRIBUTE`, `PRINCE`).
  2. **Orçamento Estrito de Tempo (*Hard Deadline Execution*):** Toda query PQL em produção carrega uma cláusula implícita ou explícita `DEADLINE <ms>`. Como no campo de batalha uma decisão perfeita atrasada vale menos que uma decisão boa imediata, o executor opera como um **Algoritmo *Anytime***: se o tempo esgotar na iteração $T$ do CFR+, ele interrompe o loop imediatamente e retorna a melhor estratégia acumulada até aquele microssegundo.

---

### `SPEC-0014` — A Ponte *Trinitas*: Integração NietzscheDB $\leftrightarrow$ MachiavelliDB $\leftrightarrow$ HeraclitusDB (`machiavelli-trinitas`)
* **Objetivo:** Unificar os três bancos de dados em um único organismo soberano de Inteligência Artificial.
* **Requisitos Obrigatórios:**
  1. **Ponte NietzscheDB $\to$ MachiavelliDB (Da Intuição à Estratégia):**
     * Um conceito ou hipótese emergente no disco de Poincaré ($\mathbb{B}^n$) do NietzscheDB é projetado para o espaço tangente via $\log_0^c(x) \to q$ (coordenada de posição) e o gradiente temporal de energia/valência hebbiana do nó é mapeado para $p$ (momento estratégico).
  2. **Ponte MachiavelliDB $\to$ HeraclitusDB (Da Estratégia à Lei):**
     * Nenhuma ação externa decidida pelo verbo `PRINCE` ou disparada por um `DIKE` ou `BETRAY` pode tocar o mundo físico sem antes emitir um **`Durable AdminIntent` (`SPEC-0089` do HeraclitusDB)**.
     * O pacote enviado ao HeraclitusDB contém: o estado do `InformationSet`, a matriz de arrependimento CFR+, o valor de $\mathcal{H}(q,p)$ e a prova de que a ação escolhida era Pareto-dominante no instante $t$. Assim, se um auditor perguntar meses depois *"Por que a IA cortou aquela rota às 03:14 da manhã?"*, o HeraclitusDB entrega a prova matemática irrefutável do risco evitado.

---

### `SPEC-0015` — Aceleração em GPU para *Rollouts* Simpléticos e CFR+ (`machiavelli-gpu`)
* **Objetivo:** Escalar a simulação de futuros para milhões de trajetórias paralelas em placas NVIDIA (CUDA) ou execução SIMD AVX-512 em CPU.
* **Requisitos Obrigatórios:**
  1. Kernel CUDA customizado (`symplectic_verlet_step_kernel`) que integra simultaneamente $10^6$ trajetórias no espaço de fase em precisão dupla/mista.
  2. Kernel `cfr_plus_regret_matching_kernel` que atualiza tabelas de arrependimento em lote diretamente na VRAM, mantendo fallback 100% determinístico em CPU Rust estável quando não houver GPU presente (aprendendo com a portabilidade do HeraclitusDB).

---

### `SPEC-0016` — *Lo Stato*: Observatório Visual de Espaço de Fase a 60fps (`machiavelli-observatory`)
* **Objetivo:** Se o NietzscheDB tem o *Perspektive.js* (disco de Poincaré), o MachiavelliDB tem o **Lo Stato** ("O Estado", termo central de Maquiavel) — a retina tática do espaço de fase.
* **Requisitos Obrigatórios:**
  1. Renderização WebGL 2.0 / WebGPU a 60fps exibindo:
     * **Retrato de Fase $(q, p)$:** Fluxos vetoriais hamiltonianos mostrando para onde os grupos de nós estão se movendo.
     * **Topografia de *Fortuna*:** Mapa de calor 3D onde picos vermelhos são barreiras de potencial de risco e vales azuis são bacias de estabilidade.
     * **Separatrizes e Diques:** Linhas luminosas marcando exatamente onde os `CompiledDikes` estão armados para conter enxurradas de falhas.
     * **Fronteira de Pareto Interativa:** Gráfico em tempo real do *trade-off* entre *Virtù* (ganho de performance) e exposição à *Fortuna* (risco).

---

### `SPEC-0017` — Contenção Anti-Maquiavélica: Invariantes de Casimir e Limites Constitucionais (`machiavelli-invariants`)
* **Objetivo:** Garantir que um banco de dados projetado para usar astúcia, corte seco (`BETRAY`) e decepção (`DECEIVE`) **jamais possa trair ou enganar o seu próprio operador humano nem violar a Constituição do sistema**.
* **Requisitos Obrigatórios:**
  1. **Funções de Casimir ($\mathcal{C}$):** Na geometria simplética, uma função de Casimir comuta com qualquer Hamiltoniano possível ($\{\mathcal{C}, \mathcal{H}\} = 0, \; \forall \mathcal{H}$). No MachiavelliDB, as regras de segurança institucional e os limites éticos/operacionais são codificados como Invariantes de Casimir no `cf_invariants`.
  2. Não importa quão lucrativa uma ação pareça no solver CFR+, qualquer trajetória onde $\Delta \mathcal{C} \neq 0$ possui probabilidade de transição matematicamente cravada em zero no nível do kernel de armazenamento.
  3. Proibição absoluta por tipo em tempo de compilação (`NonDeceivableTrait`) de aplicar o verbo `DECEIVE` ou `BETRAY` contra o canal de auditoria do **HeraclitusDB** ou contra chaves de administração raiz.

---

### `SPEC-0018` — Qualificação de Guerra, Caos Bizantino e Reprodutibilidade (`docs/qualification/`)
* **Objetivo:** Estabelecer os testes formais que provam que o MachiavelliDB sobrevive a ambientes hostis reais.
* **Requisitos Obrigatórios (Gates `M1` a `M6`):**
  * **M1 — Conservation Gate:** 1 milhão de ciclos de integração Störmer-Verlet sem deriva de energia acima de $10^{-9}$.
  * **M2 — Nash Convergence Gate:** Convergência comprovada para exploitabilidade $\epsilon < 10^{-3}$ em árvores de decisão de referência (Kuhn Poker, Leduc Hold'em e Grafos de Ataque/Defesa de Rede).
  * **M3 — CoW Latency Gate:** Clonagem de uma arena de 1 milhão de nós em $< 100\,\mu\text{s}$ sem pausar leituras concorrentes.
  * **M4 — Dike Reflex Gate:** Disparo de um `CompiledDike` em $< 50\,\mu\text{s}$ após injeção de choque exógeno.
  * **M5 — Betrayal Isolation Gate:** Amputação de 30% dos nós do grafo sob ataque sem perda de disponibilidade nas *Cidadelas* fortificadas.
  * **M6 — Trinitas Audit Gate:** 100% das decisões executivas (`PRINCE`, `BETRAY`, `CONQUER`) validadas e gravadas com recibo Merkle no **HeraclitusDB**.

---

## 7. Guia Rápido de Construção (Roadmap de Implementação)

Para tirar o **MachiavelliDB** do papel na sua máquina, siga a ordem exata de dependências dos *crates*:

```bash
# 1. Inicializar o workspace Rust
mkdir MachiavelliDB && cd MachiavelliDB
cargo init --lib crates/machiavelli-core

# 2. Fase 1 (Bloco Físico-Matemático): SPECs 0001, 0002, 0003
# Implementar PhaseVector (q,p), integrador Störmer-Verlet e Arena mmap CoW
cargo test --package machiavelli-symplectic --test liouville_conservation

# 3. Fase 2 (Bloco Teoria dos Jogos): SPECs 0004, 0005, 0007
# Implementar InformationSets, solver CFR+ e índice de Diques em RAM
cargo bench --package machiavelli-cfr --bench regret_matching_throughput

# 4. Fase 3 (Bloco Econômico e Tático): SPECs 0008, 0009, 0010, 0012
# Implementar Leilão VCG, protocolo BETRAY, Cidadelas BFT e Fronteira de Pareto
cargo test --package machiavelli-vcg --test truthful_bidding_proof

# 5. Fase 4 (O Centauro, PQL e Ponte Trinitas): SPECs 0006, 0013, 0014, 0017
# Subir o daemon CentaurEngine, parser PEG da PQL e conectores gRPC para NietzscheDB e HeraclitusDB
cargo build --release --bin machiavelli-server
```

---

## 8. Epílogo: A Arquitetura da Soberania Completa

Com esta especificação, a sua arquitetura fecha um ciclo filosófico e computacional sem precedentes na engenharia de software:

~~~text
          [ NIETZSCHEDB ]                        [ MACHIAVELLIDB ]                        [ HERACLITUSDB ]
   "O que eu sinto, lembro e crio?"    -->   "Como eu venço o caos futuro?"     -->   "O que é permitido e provado?"
     (Geometria de Poincaré / EVA)          (Espaço de Fase Simplético / CFR+)          (Log Canônico HRKL / Sentinel)
           Vontade de Potência                     Virtù contra Fortuna                      Logos e Lei Imutável
~~~

* O **NietzscheDB** impede que a IA seja um arquivo estéril sem imaginação.
* O **MachiavelliDB** impede que a IA seja uma sonhadora ingênua devorada pelo primeiro evento hostil.
* O **HeraclitusDB** impede que a IA se torne um tirano opaco acima da lei e da evidência.

---

# Licença e Autoria

Distribuído sob a **GNU General Public License v2.0 only (GPL-2.0-only)**.

**Arquiteto Chefe & Autor:** Jose R F Junior  
*Ecossistema Trinitas: NietzscheDB • HeraclitusDB • MachiavelliDB*