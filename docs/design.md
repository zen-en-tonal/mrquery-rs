# mrquery-rs 完全設計書

## 0. 文書の目的

本書は、`mrquery-rs` を実装するための技術設計書である。

本プロジェクトは、深層学習を使わずに画像検索を行う Rust 製検索コアを実装する。
将来的に Elixir アプリケーションから利用されることを前提とし、Rust 側ではインフラ、分散制御、HTTP API、Raft 実装を持たない。

本書は、LLM にコード生成を依頼する前提で書かれている。
そのため、単なる設計方針だけではなく、以下を明示する。

* 実装範囲
* 非実装範囲
* crate 構成
* API 境界
* データ型
* ストレージ形式
* インデックス構造
* クエリ処理
* Elixir 連携方針
* Raft 連携を想定した状態モデル
* 実装フェーズ
* テスト条件
* ベンチマーク条件
* LLM に実装させる際の作業分解
* 受け入れ条件

---

# 1. プロジェクト概要

## 1.1 プロジェクト名

`mrquery-rs`

## 1.2 目的

`mrquery-rs` は、深層学習を使わずに、画像の視覚的特徴に基づく高速検索を行う Rust 製ライブラリおよび worker である。

本プロジェクトは、以下の特徴量を組み合わせる。

* Haar wavelet による多重解像度署名
* 色特徴
* エッジ特徴
* perceptual hash
* 将来的な局所特徴
* 将来的な部分画像検索用 region signature

中心思想は以下である。

```text
画像そのものを比較しない。
検索に向いた小さな署名へ変換し、
署名を索引化して高速に検索する。
```

## 1.3 深層学習を使わない理由

本プロジェクトでは、以下を使用しない。

* CNN
* Vision Transformer
* CLIP
* DINO
* SigLIP
* 画像言語モデル
* embedding model
* deep metric learning

理由は以下である。

* ローカル環境で完結させる
* モデルファイルに依存しない
* 実装と動作を説明可能にする
* 軽量にする
* Rust 側を検索カーネルとして安定させる
* Elixir 側から管理しやすくする

---

# 2. システム全体方針

## 2.1 Rust 側の責務

Rust 側は、検索コアに限定する。

Rust 側が担当するもの:

* 画像読み込み
* 画像正規化
* 特徴抽出
* 画像署名生成
* インデックス作成
* インデックス更新
* クエリ実行
* スコア計算
* versioned storage の読み書き
* snapshot manifest の生成
* WriteBatch の適用
* Elixir Port から呼べる worker protocol

## 2.2 Rust 側で実装しないもの

Rust 側では以下を実装しない。

* HTTP API
* gRPC API
* WebSocket
* 認証
* 認可
* Rate limit
* Raft
* leader election
* follower replication
* quorum commit
* cluster membership
* ノード発見
* ジョブキュー
* 管理 UI
* 監視サーバ
* メトリクス集約
* 永続的な分散ログ

これらは Elixir 側または外部インフラで担当する。

## 2.3 Elixir 側の責務

Elixir 側は制御プレーンである。

Elixir 側が担当する想定のもの:

* HTTP / gRPC API
* 認証認可
* request routing
* clustering
* Raft
* job queue
* supervision
* back pressure
* metrics
* logging
* snapshot 転送
* Rust worker の起動、監視、再起動
* read consistency 管理
* write coordination

## 2.4 基本アーキテクチャ

```text
Clients
  ↓
Elixir API Layer
  ↓
Elixir Control Plane
  - Routing
  - Raft
  - Supervision
  - Jobs
  ↓
Rust Worker
  - Index API
  - Query API
  ↓
Versioned Index Storage
```

---

# 3. 設計原則

## 3.1 原則一覧

本プロジェクトは以下の原則に従う。

1. Rust 側は検索コアに集中する
2. Rust 側は分散制御を知らない
3. Rust 側は deterministic な state machine として振る舞う
4. すべての index commit は `IndexVersion` を持つ
5. query は必ず `IndexVersion` を返す
6. write は `WriteBatch` 単位で適用する
7. snapshot はファイル集合と manifest で表現する
8. reader と writer は分離する
9. 初期実装では正しさを優先する
10. 高速化は測定後に行う
11. LLM が暴走しないよう、実装単位を小さく保つ

---

# 4. 非目標

以下は本プロジェクトの非目標である。

## 4.1 検索機能としての非目標

* 自然言語画像検索
* 意味的類似検索
* 顔認識
* 人物同定
* OCR
* 動画全体理解
* 画像生成
* deep embedding search

## 4.2 システム機能としての非目標

* Rust 製 API サーバ
* Rust 製 Raft
* Rust 製 distributed storage
* Rust 製 task queue
* Rust 製 dashboard
* Rust 側での user account 管理

## 4.3 MVP で扱わないもの

* 部分画像検索
* 局所特徴
* mmap 最適化
* compaction
* NIF
* FFI
* segment merge
* incremental update

---

# 5. 想定ユースケース

## 5.1 画像全体検索

画像をクエリとして、見た目が似ている画像を検索する。

```bash
mrq query --db ./db --image ./query.png --mode image --top-k 20
```

## 5.2 スキャン画像検索

低品質スキャン、縮小画像、圧縮劣化画像から元画像または類似画像を検索する。

## 5.3 近似重複検索

リサイズ、軽い JPEG 圧縮、軽い色変化を受けた画像を検出する。

```bash
mrq query --db ./db --image ./query.jpg --mode duplicate --top-k 20
```

## 5.4 スケッチ検索

手描きスケッチや線画をクエリとして、輪郭や構図が近い画像を検索する。

```bash
mrq query --db ./db --image ./sketch.png --mode sketch --top-k 20
```

## 5.5 Elixir からの worker 利用

Elixir は Rust worker を Port 経由で起動し、JSON Lines protocol で command を送る。

```text
Elixir process
  ↓ JSONL request
mrquery-worker
  ↓
QueryEngine / IndexWriter
  ↓ JSONL response
Elixir process
```

---

# 6. Rust Workspace 構成

## 6.1 最終構成

```text
mrquery-rs/
  Cargo.toml
  README.md

  crates/
    mrq-core/
      src/
        lib.rs
        error.rs
        image_norm.rs
        color.rs
        wavelet.rs
        edge.rs
        hash.rs
        signature.rs
        score.rs
        config.rs

    mrq-index/
      src/
        lib.rs
        error.rs
        types.rs
        postings.rs
        hash_index.rs
        store.rs
        version.rs
        snapshot.rs
        writer.rs
        reader.rs
        batch.rs

    mrq-query/
      src/
        lib.rs
        engine.rs
        candidate.rs
        rank.rs
        explain.rs

    mrq-worker/
      src/
        main.rs
        protocol.rs
        commands.rs
        jsonl.rs

    mrq-cli/
      src/
        main.rs
        commands/
          index.rs
          query.rs
          inspect.rs
          bench.rs
          snapshot.rs

    mrq-eval/
      src/
        lib.rs
        dataset.rs
        metrics.rs
        report.rs

    mrq-ffi/
      src/
        lib.rs

  docs/
    design.md
    storage-format.md
    worker-protocol.md
    evaluation.md
    llm-implementation-plan.md

  tests/
    fixtures/
      images/
      queries/
      truth.json

  benches/
    wavelet.rs
    search.rs
    indexing.rs
```

## 6.2 Crate 役割

### `mrq-core`

画像処理と特徴抽出を担当する。
ストレージや worker protocol には依存しない。

担当:

* 画像正規化
* Haar wavelet
* 色特徴
* エッジ特徴
* perceptual hash
* signature 生成
* score 計算の基本部品

### `mrq-index`

インデックス作成、保存、更新、snapshot を担当する。

担当:

* versioned storage
* `IndexWriter`
* `IndexReader`
* `WriteBatch`
* postings
* hash buckets
* manifest
* snapshot manifest

### `mrq-query`

検索実行を担当する。

担当:

* `QueryEngine`
* candidate generation
* scoring
* reranking
* explanation

### `mrq-worker`

Elixir Port 用 worker を担当する。

担当:

* JSON Lines protocol
* request parsing
* response writing
* worker command dispatch
* long-running process mode

### `mrq-cli`

手動操作と検証用 CLI。

担当:

* index
* query
* inspect
* bench
* snapshot

### `mrq-eval`

評価用。

担当:

* ground truth 読み込み
* Recall@k
* latency
* ablation report

### `mrq-ffi`

将来用。
MVP では空または最小限でよい。

---

# 7. 依存クレート方針

## 7.1 初期依存

```toml
[workspace.dependencies]
anyhow = "1"
thiserror = "2"
image = "0.25"
rayon = "1"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
bincode = "1"
toml = "0.8"
tracing = "0.1"
tracing-subscriber = "0.3"
crc32fast = "1"
sha2 = "0.10"
tempfile = "3"
walkdir = "2"
```

## 7.2 後で検討する依存

```toml
memmap2 = "0.9"
rmp-serde = "1"
criterion = "0.5"
```

## 7.3 依存追加ルール

LLM は勝手に大型依存を追加してはならない。

依存を追加してよい条件:

* 標準ライブラリで実装すると明らかに危険
* 画像処理や serialization で事実上必要
* crate の責務に合っている
* 追加理由を PR または実装ログに明記する

依存を追加してはいけない例:

* Web framework
* Raft library
* database server client
* async runtime
* deep learning runtime
* GPU library

---

# 8. 主要データ型

## 8.1 ImageId

```rust
pub type ImageId = u64;
```

## 8.2 IndexVersion

```rust
pub type IndexVersion = u64;
```

すべての commit で単調増加する。

## 8.3 BatchId

```rust
pub type BatchId = String;
```

Elixir 側の Raft log entry と対応する。

## 8.4 ImageDocument

```rust
pub struct ImageDocument {
    pub image_id: ImageId,
    pub external_id: String,
    pub source: ImageSource,
    pub metadata: DocumentMetadata,
}
```

```rust
pub enum ImageSource {
    Path(String),
    Bytes(Vec<u8>),
}
```

## 8.5 DocumentMetadata

```rust
pub struct DocumentMetadata {
    pub original_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub content_type: Option<String>,
    pub user_metadata_json: Option<String>,
}
```

`user_metadata_json` は Rust 側では解釈しない。
Elixir 側が自由に利用できる opaque metadata として扱う。

## 8.6 NormalizedImage

```rust
pub struct NormalizedImage {
    pub width: u32,
    pub height: u32,
    pub pixels_rgb_f32: Vec<[f32; 3]>,
}
```

MVP では 128 x 128 固定を基本とする。

## 8.7 WaveletToken

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct WaveletToken {
    pub channel: u8,
    pub scale: u8,
    pub band: u8,
    pub x: u16,
    pub y: u16,
    pub sign: i8,
}
```

## 8.8 ImageSignature

```rust
pub struct ImageSignature {
    pub image_id: ImageId,
    pub width: u32,
    pub height: u32,
    pub avg_color: [f32; 3],
    pub wavelet_tokens: Vec<WaveletToken>,
    pub color_hist: Vec<u16>,
    pub edge_hist: Vec<u16>,
    pub phash: u64,
}
```

## 8.9 WriteOperation

```rust
pub enum WriteOperation {
    Add(ImageDocument),
    Remove { image_id: ImageId },
    Update(ImageDocument),
}
```

## 8.10 WriteBatch

```rust
pub struct WriteBatch {
    pub batch_id: BatchId,
    pub base_index_version: IndexVersion,
    pub operations: Vec<WriteOperation>,
}
```

## 8.11 SearchResult

```rust
pub struct SearchResult {
    pub image_id: ImageId,
    pub external_id: String,
    pub score: f32,
    pub rank: usize,
    pub explanation: Option<SearchExplanation>,
}
```

## 8.12 SearchExplanation

```rust
pub struct SearchExplanation {
    pub wavelet_matches: usize,
    pub color_distance: f32,
    pub edge_similarity: f32,
    pub hash_distance: u32,
    pub aspect_ratio_penalty: f32,
}
```

---

# 9. 画像正規化

## 9.1 目的

入力画像のサイズ、色形式、向きを揃える。

## 9.2 MVP 手順

```text
画像読み込み
  ↓
RGB 変換
  ↓
長辺を指定サイズにリサイズ
  ↓
正方形キャンバスに中央配置
  ↓
128 x 128 RGB f32 に変換
```

## 9.3 初期設定

```toml
[image]
size = 128
background = [1.0, 1.0, 1.0]
```

## 9.4 LLM 実装指示

* まず EXIF orientation は扱わなくてよい
* panic させない
* decode error は `MrqError::Decode` に変換する
* 入力画像が極端に大きい場合の制限を入れる
* 正規化結果は deterministic であること

---

# 10. 特徴抽出

## 10.1 Wavelet Signature

### 10.1.1 目的

画像の大域構造を多重解像度で表現する。

### 10.1.2 MVP

* Haar wavelet のみ
* RGB 各チャネルまたは輝度チャネル
* 上位 K 個の係数を token 化
* 初期 K は 64

### 10.1.3 処理

```text
NormalizedImage
  ↓
channel extraction
  ↓
Haar transform
  ↓
coefficient ranking by abs value
  ↓
top-k selection
  ↓
WaveletToken generation
```

### 10.1.4 制約

* token は deterministic に並ぶこと
* 絶対値が同じ場合は位置順で tie-break する
* sign は `-1` または `1`
* 0 係数は原則として token 化しない

## 10.2 Color Signature

### 10.2.1 MVP

* 平均 RGB
* RGB histogram
* channel ごとに 8 bins

### 10.2.2 距離

* 平均色: L2
* histogram: L1

## 10.3 Edge Signature

### 10.3.1 MVP

* Sobel edge
* 方向 bins: 8
* grid: 4 x 4
* 次元: 128

### 10.3.2 Sketch mode

スケッチ検索では edge の重みを高くする。

## 10.4 Perceptual Hash

### 10.4.1 MVP

簡易 pHash を実装する。

推奨手順:

```text
grayscale
  ↓
32 x 32 resize
  ↓
DCT or simple low frequency transform
  ↓
8 x 8 low frequency coefficients
  ↓
median threshold
  ↓
u64 hash
```

DCT 実装が重い場合、初期実装では average hash に落としてもよい。
ただし型名は `PerceptualHash` とし、実装種類を config で持つ。

### 10.4.2 Hamming distance

```rust
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}
```

---

# 11. スコア設計

## 11.1 基本式

```text
Score(Q, T)
=
  w_wavelet * WaveletMatch(Q, T)
- w_color   * ColorDistance(Q, T)
+ w_edge    * EdgeSimilarity(Q, T)
- w_hash    * HammingDistance(Q, T)
- w_aspect  * AspectRatioPenalty(Q, T)
```

## 11.2 ScoringProfile

```rust
pub enum ScoringProfile {
    Image,
    Sketch,
    Duplicate,
}
```

## 11.3 初期重み

```toml
[scoring.image]
wavelet = 1.00
color = 0.25
edge = 0.50
hash = 0.15
aspect = 0.10

[scoring.sketch]
wavelet = 0.60
color = 0.05
edge = 1.00
hash = 0.05
aspect = 0.20

[scoring.duplicate]
wavelet = 0.40
color = 0.30
edge = 0.10
hash = 1.00
aspect = 0.30
```

## 11.4 LLM 実装指示

* まずは単純なスコアでよい
* score の単位が揃っていなくても MVP では許容する
* ただし各 component score を explanation に出せるようにする
* ranking は score 降順
* tie-break は `image_id` 昇順

---

# 12. インデックス構造

## 12.1 MVP

MVP では線形検索を実装する。

```text
Vec<ImageSignature>
  ↓
全件 score
  ↓
top-k
```

理由:

* 特徴量の正しさを確認できる
* LLM 実装の複雑度を下げる
* バグの切り分けが容易

## 12.2 v0.2 Wavelet Inverted Index

```text
WaveletToken -> Vec<Posting>
```

```rust
pub struct Posting {
    pub image_id: ImageId,
    pub weight: f32,
}
```

検索:

```text
query tokens
  ↓
posting list lookup
  ↓
score accumulation
  ↓
candidate set
```

## 12.3 Hash Index

```text
hash_prefix -> Vec<ImageId>
```

初期:

* prefix bits: 16
* 同一 prefix bucket のみを見る

将来:

* 近傍 prefix
* Multi-Index Hashing
* Hamming ball search

---

# 13. Versioned Storage

## 13.1 方針

Raft 連携しやすいように、storage は versioned directory とする。

## 13.2 ディレクトリ構成

```text
db/
  CURRENT
  versions/
    0000000001/
      manifest.json
      metadata.jsonl
      signatures.bin
      wavelet_postings.bin
      hash_buckets.bin
      deletes.bin
    0000000002/
      manifest.json
      metadata.jsonl
      signatures.bin
      wavelet_postings.bin
      hash_buckets.bin
      deletes.bin
```

## 13.3 CURRENT

`CURRENT` は現在有効な version を示すテキストファイルである。

内容例:

```text
0000000002
```

## 13.4 Commit 手順

```text
1. target version directory を作成
2. index files を書き込む
3. checksum を計算
4. manifest.json を書き込む
5. fsync する
6. CURRENT.tmp を書く
7. CURRENT.tmp を CURRENT に atomic rename する
```

## 13.5 不変条件

* `CURRENT` が指す version directory は完全でなければならない
* query reader は `CURRENT` が指す complete version のみを開く
* commit 中の directory を reader が読んではならない
* manifest の checksum が合わない version は開いてはならない
* `IndexVersion` は単調増加する

---

# 14. Manifest

## 14.1 Version Manifest

```json
{
  "format_version": 1,
  "index_version": 2,
  "created_at_unix_ms": 1730000000000,
  "image_size": 128,
  "wavelet_top_k": 64,
  "feature_config_hash": "sha256:...",
  "files": [
    {
      "path": "metadata.jsonl",
      "size_bytes": 12345,
      "checksum": "sha256:..."
    },
    {
      "path": "signatures.bin",
      "size_bytes": 123456,
      "checksum": "sha256:..."
    }
  ],
  "applied_batches": [
    "batch-001",
    "batch-002"
  ]
}
```

## 14.2 File Checksum

MVP では SHA-256 を使う。

理由:

* snapshot 転送時に検証しやすい
* Elixir 側でも扱いやすい
* 実装の正しさが見やすい

---

# 15. Reader / Writer 分離

## 15.1 QueryEngine

```rust
pub struct QueryEngine {
    pub index_version: IndexVersion,
}
```

責務:

* read-only index を開く
* query を実行する
* search result を返す

QueryEngine は writer を持たない。

## 15.2 IndexWriter

```rust
pub struct IndexWriter {
    pub base_version: IndexVersion,
    pub target_version: IndexVersion,
}
```

責務:

* WriteBatch を適用する
* 新しい version directory を作る
* commit する

IndexWriter は query を実行しない。

---

# 16. WriteBatch と Raft 連携

## 16.1 方針

Rust 側は Raft を知らない。
Elixir 側が合意済みの WriteBatch を Rust に渡す。

```text
Elixir Raft committed log entry
  ↓
WriteBatch
  ↓
Rust apply_batch
  ↓
new IndexVersion
```

## 16.2 Idempotency

同じ `batch_id` が二重に適用されてはならない。

Rust 側は manifest に適用済み batch を記録する。

MVP では直近すべての batch ID を manifest に保持してよい。
将来は compaction により圧縮する。

## 16.3 base_index_version

`WriteBatch.base_index_version` は、適用対象の version を示す。

ルール:

* 現在 version と一致する場合、適用する
* すでに batch_id が適用済みなら no-op
* base version が古く、batch 未適用なら error
* base version が未来なら error

## 16.4 ApplyResult

```rust
pub struct ApplyResult {
    pub batch_id: BatchId,
    pub previous_version: IndexVersion,
    pub new_version: IndexVersion,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}
```

---

# 17. Snapshot

## 17.1 目的

Elixir 側の Raft snapshot と連携しやすくする。

## 17.2 SnapshotManifest

```rust
pub struct SnapshotManifest {
    pub index_version: IndexVersion,
    pub format_version: u32,
    pub files: Vec<SnapshotFile>,
    pub checksum: String,
}
```

```rust
pub struct SnapshotFile {
    pub path: String,
    pub size_bytes: u64,
    pub checksum: String,
}
```

## 17.3 create_snapshot

```rust
pub fn create_snapshot(
    index_path: &Path,
    output_path: &Path,
) -> Result<SnapshotManifest>;
```

## 17.4 load_snapshot

```rust
pub fn load_snapshot(
    snapshot_path: &Path,
    index_path: &Path,
) -> Result<IndexVersion>;
```

## 17.5 MVP 方針

MVP では snapshot は tar archive でなくてもよい。
version directory をコピーし、manifest を出すだけでよい。

将来 Elixir 側で archive 化する。

---

# 18. Query API

## 18.1 QueryEngine API

```rust
impl QueryEngine {
    pub fn open(index_path: impl AsRef<Path>) -> Result<Self>;

    pub fn query_by_image(
        &self,
        query: ImageQuery,
        options: QueryOptions,
    ) -> Result<QueryResponse>;

    pub fn query_by_signature(
        &self,
        signature: ImageSignature,
        options: QueryOptions,
    ) -> Result<QueryResponse>;

    pub fn explain_result(
        &self,
        query: ImageQuery,
        image_id: ImageId,
    ) -> Result<SearchExplanation>;
}
```

## 18.2 ImageQuery

```rust
pub struct ImageQuery {
    pub source: ImageSource,
    pub mode: QueryMode,
}
```

```rust
pub enum QueryMode {
    Image,
    Sketch,
    Duplicate,
    Region,
}
```

MVP では `Region` は受け付ける必要はない。
指定された場合は `Unsupported` error を返す。

## 18.3 QueryOptions

```rust
pub struct QueryOptions {
    pub top_k: usize,
    pub candidate_limit: usize,
    pub scoring_profile: ScoringProfile,
    pub include_explanation: bool,
}
```

## 18.4 QueryResponse

```rust
pub struct QueryResponse {
    pub index_version: IndexVersion,
    pub results: Vec<SearchResult>,
    pub stats: QueryStats,
}
```

## 18.5 QueryStats

```rust
pub struct QueryStats {
    pub candidates: usize,
    pub elapsed_ms: f32,
}
```

## 18.6 Query 不変条件

* 結果は score 降順
* score が同じ場合は image_id 昇順
* `top_k` を超えて返さない
* `index_version` を必ず返す
* 削除済み image_id を返してはならない
* 読み込み中に index version が変わっても、開いている QueryEngine の version は変わらない

---

# 19. Index API

## 19.1 IndexWriter API

```rust
impl IndexWriter {
    pub fn create(index_path: impl AsRef<Path>, config: IndexConfig) -> Result<Self>;

    pub fn open(index_path: impl AsRef<Path>) -> Result<Self>;

    pub fn apply_batch(&mut self, batch: WriteBatch) -> Result<ApplyResult>;

    pub fn commit(&mut self) -> Result<CommitResult>;

    pub fn create_snapshot(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<SnapshotManifest>;
}
```

## 19.2 MVP 注意

MVP では `apply_batch` の中で full rebuild してよい。
高速化は後で行う。

---

# 20. Worker Protocol

## 20.1 方針

Elixir Port から呼び出しやすくするため、Rust worker は JSON Lines protocol を提供する。

## 20.2 起動

```bash
mrq-worker --db ./db
```

## 20.3 Request Envelope

```json
{
  "request_id": "req-001",
  "command": "query",
  "payload": {}
}
```

## 20.4 Response Envelope

```json
{
  "request_id": "req-001",
  "ok": true,
  "payload": {}
}
```

## 20.5 Error Response

```json
{
  "request_id": "req-001",
  "ok": false,
  "error": {
    "kind": "decode_error",
    "message": "failed to decode image"
  }
}
```

## 20.6 Commands

### query

```json
{
  "request_id": "req-001",
  "command": "query",
  "payload": {
    "image_path": "./query.png",
    "mode": "image",
    "top_k": 20,
    "include_explanation": false
  }
}
```

### apply_batch

```json
{
  "request_id": "req-002",
  "command": "apply_batch",
  "payload": {
    "batch_id": "batch-001",
    "base_index_version": 1,
    "operations": [
      {
        "type": "add",
        "image_id": 123,
        "external_id": "img-123",
        "path": "./images/img123.jpg",
        "metadata": {}
      }
    ]
  }
}
```

### create_snapshot

```json
{
  "request_id": "req-003",
  "command": "create_snapshot",
  "payload": {
    "output_path": "./snapshot"
  }
}
```

### reload

```json
{
  "request_id": "req-004",
  "command": "reload",
  "payload": {}
}
```

`reload` は `CURRENT` を読み直し、新しい QueryEngine を開く。

## 20.7 Worker 不変条件

* 1 request に必ず 1 response
* panic してはならない
* 不正 JSON には error response を返す
* stdout には response JSONL 以外を書かない
* log は stderr に出す
* request_id はそのまま返す

---

# 21. CLI

## 21.1 index

```bash
mrq index \
  --input ./images \
  --db ./db \
  --size 128 \
  --wavelet-k 64
```

## 21.2 query

```bash
mrq query \
  --db ./db \
  --image ./query.png \
  --mode image \
  --top-k 20
```

## 21.3 inspect

```bash
mrq inspect \
  --db ./db \
  --image-id 123
```

## 21.4 snapshot

```bash
mrq snapshot create \
  --db ./db \
  --output ./snapshot
```

## 21.5 bench

```bash
mrq bench \
  --db ./db \
  --queries ./queries \
  --ground-truth ./truth.json
```

---

# 22. 設定ファイル

## 22.1 TOML

```toml
[image]
size = 128
max_input_pixels = 40000000

[wavelet]
kind = "haar"
top_k = 64
channels = "rgb"

[color]
hist_bins = 8

[edge]
method = "sobel"
orientation_bins = 8
grid = 4

[hash]
kind = "average"
bits = 64
prefix_bits = 16

[scoring.image]
wavelet = 1.00
color = 0.25
edge = 0.50
hash = 0.15
aspect = 0.10

[scoring.sketch]
wavelet = 0.60
color = 0.05
edge = 1.00
hash = 0.05
aspect = 0.20

[scoring.duplicate]
wavelet = 0.40
color = 0.30
edge = 0.10
hash = 1.00
aspect = 0.30
```

---

# 23. Error 設計

## 23.1 Error enum

```rust
pub enum MrqError {
    Io,
    Decode,
    Config,
    Unsupported,
    InvalidRequest,
    IndexCorrupt,
    VersionMismatch,
    ChecksumMismatch,
    NotFound,
    Internal,
}
```

実装では `thiserror` を使う。

## 23.2 Error 方針

* library は panic しない
* CLI は error を人間向けに表示する
* worker は machine-readable error を返す
* checksum mismatch は必ず error
* unsupported mode は error
* invalid top_k は error

---

# 24. テスト設計

## 24.1 Unit Tests

### `mrq-core`

* 正規化結果が指定サイズになる
* 同じ画像から同じ signature が得られる
* Haar transform が deterministic
* WaveletToken の tie-break が deterministic
* hash distance が正しい
* edge hist の次元が正しい
* score tie-break が正しい

### `mrq-index`

* version directory が作られる
* CURRENT が正しく切り替わる
* manifest checksum が検証される
* base version mismatch で error
* 同じ batch_id の再適用が no-op
* remove 後に query result に出ない

### `mrq-query`

* top_k 件以下を返す
* score 降順で返す
* image_id tie-break が効く
* index_version が返る

### `mrq-worker`

* 正常 request に response
* 不正 JSON に error
* request_id が保持される
* stdout に JSONL 以外を書かない

## 24.2 Integration Tests

fixture:

```text
tests/fixtures/
  images/
    img001.png
    img002.png
    img003.png
  queries/
    q001.png
  truth.json
```

テスト:

* CLI で index 作成
* CLI で query
* worker で query
* snapshot 作成
* snapshot load
* batch apply
* duplicate batch no-op

## 24.3 Property-like Tests

可能なら以下を確認する。

* 同じ batch を2回適用しても version が壊れない
* query result は常に deterministic
* version switch 中に incomplete version を開かない

---

# 25. ベンチマーク

## 25.1 MVP benchmark

* 画像1枚の正規化時間
* signature 抽出時間
* 1万枚 index 作成時間
* 1万枚線形 query latency

## 25.2 v0.2 benchmark

* posting lookup
* candidate generation
* hash distance batch
* 10万枚 query latency

## 25.3 指標

```text
Recall@1
Recall@10
Recall@20
Median Rank
P50 Latency
P95 Latency
Index Size
Build Throughput
```

---

# 26. セキュリティと堅牢性

## 26.1 入力画像

* 最大画素数を制限する
* decode error を握り潰さない
* path traversal を避ける
* unsupported format は error
* panic しない

## 26.2 Worker

* stdout は protocol 専用
* stderr に log
* malformed request で process を落とさない
* request size limit を検討する
* bytes 入力は MVP では後回しでもよい

## 26.3 Storage

* checksum 検証
* atomic CURRENT switch
* incomplete version を開かない
* manifest がない version を開かない
* format_version mismatch を error にする

---

# 27. LLM 実装ルール

## 27.1 LLM に守らせる絶対ルール

LLM は以下を守ること。

1. 一度に巨大な実装をしない
2. crate 境界を勝手に変えない
3. Rust 側に HTTP server を追加しない
4. Rust 側に Raft を追加しない
5. 深層学習依存を追加しない
6. panic を通常エラー処理に使わない
7. `unwrap()` を本番コードに残さない
8. テストなしで機能完了としない
9. stdout に worker protocol 以外を出さない
10. public API の型を変更したら設計書も更新する

## 27.2 LLM に許可すること

* 内部 helper 関数の追加
* private struct の追加
* テスト fixture の追加
* 小さな依存の追加
* エラー型の細分化
* CLI option の追加

ただし、設計意図を壊さない範囲に限る。

## 27.3 コードスタイル

* `cargo fmt` が通ること
* `cargo clippy` が重大警告なしで通ること
* public item には最低限の doc comment を付ける
* 関数は短く保つ
* IO と pure logic を分ける
* deterministic な処理を優先する

---

# 28. LLM 実装フェーズ

## Phase 0: Workspace Skeleton

### 実装内容

* workspace 作成
* crate 作成
* 共通 error
* config 型
* 空の CLI
* 空の worker

### 受け入れ条件

```bash
cargo check --workspace
cargo test --workspace
cargo run -p mrq-cli -- --help
cargo run -p mrq-worker -- --help
```

---

## Phase 1: Core Signature MVP

### 実装内容

* image load
* normalize to 128 x 128
* avg color
* simple color hist
* Haar wavelet
* top-k token
* simple average hash
* ImageSignature

### 受け入れ条件

* 同じ画像から同じ signature
* signature に wavelet token が含まれる
* unit test が通る

---

## Phase 2: Linear Index MVP

### 実装内容

* metadata 保存
* signatures 保存
* versioned directory 作成
* CURRENT 作成
* QueryEngine open
* linear search
* query response

### 受け入れ条件

```bash
mrq index --input ./tests/fixtures/images --db ./tmp/db
mrq query --db ./tmp/db --image ./tests/fixtures/queries/q001.png --top-k 3
```

---

## Phase 3: Worker Protocol MVP

### 実装内容

* JSONL request parser
* query command
* error response
* reload command

### 受け入れ条件

* stdin に query request を渡すと stdout に response が1行出る
* 不正 JSON で worker が落ちない
* request_id が維持される

---

## Phase 4: WriteBatch

### 実装内容

* WriteBatch 型
* Add
* Remove
* Update
* apply_batch
* batch_id idempotency
* version increment
* full rebuild commit

### 受け入れ条件

* batch apply で version が増える
* 同じ batch の再適用が no-op
* remove した image_id が query 結果に出ない

---

## Phase 5: Snapshot

### 実装内容

* SnapshotManifest
* create_snapshot
* load_snapshot
* checksum
* CLI snapshot command
* worker snapshot command

### 受け入れ条件

* snapshot 作成後に別 db へ load できる
* load 後に query 結果が一致する

---

## Phase 6: Wavelet Inverted Index

### 実装内容

* WaveletToken -> postings
* candidate generation
* candidate_limit
* fallback linear mode
* scoring integration

### 受け入れ条件

* 線形検索と上位結果が大きく乖離しない
* 1万枚で線形より速い

---

## Phase 7: Edge and Sketch Mode

### 実装内容

* Sobel edge
* edge histogram
* sketch scoring profile
* explanation

### 受け入れ条件

* sketch mode が動く
* explanation に edge component が出る

---

## Phase 8: Segment Storage

### 実装内容

* immutable segment
* deletes
* multiple segments reader
* simple compaction placeholder

### 受け入れ条件

* add batch が新 segment になる
* delete が tombstone になる
* query は複数 segment を統合する

---

## Phase 9: Elixir Integration Hardening

### 実装内容

* stable JSON schema
* worker long-running mode
* reload
* graceful shutdown
* protocol version
* request size limit

### 受け入れ条件

* Elixir Port から安全に呼べる
* stdout protocol が安定している

---

# 29. LLM 実装タスク分割例

## Task 001: Workspace 作成

目的:

* workspace と crate を作る

完了条件:

* `cargo check --workspace`
* `cargo test --workspace`

禁止:

* 画像処理を実装しない
* index を実装しない

## Task 002: Error と Config

目的:

* 共通 error と config 型を作る

完了条件:

* TOML config を読み込める
* default config がある

## Task 003: Image Normalization

目的:

* 画像読み込みと 128 x 128 正規化

完了条件:

* unit test
* deterministic output

## Task 004: Wavelet Token

目的:

* Haar wavelet と token 化

完了条件:

* top-k token が取れる
* tie-break deterministic

## Task 005: Signature Extraction

目的:

* ImageSignature を生成

完了条件:

* avg color
* color hist
* wavelet tokens
* hash

## Task 006: Linear Query

目的:

* signature 同士を比較して top-k

完了条件:

* score 降順
* top_k 制限
* explanation optional

## Task 007: Versioned Storage MVP

目的:

* version directory に保存

完了条件:

* CURRENT switch
* manifest
* reopen できる

## Task 008: CLI MVP

目的:

* index と query

完了条件:

* fixture で end-to-end 動作

## Task 009: Worker Query

目的:

* JSONL worker で query

完了条件:

* valid request
* invalid request
* request_id preserved

## Task 010: WriteBatch

目的:

* add/remove/update

完了条件:

* idempotent
* version increment
* remove respected

---

# 30. 完了定義

## 30.1 MVP 完了定義

MVP は以下を満たしたとき完了とする。

* `cargo test --workspace` が通る
* `cargo fmt --check` が通る
* `cargo clippy --workspace` が重大警告なし
* CLI で index 作成できる
* CLI で query できる
* worker で query できる
* versioned storage が作られる
* query response に index_version が含まれる
* 同じ query の結果が deterministic
* design.md と実装が大きく矛盾していない

## 30.2 v0.2 完了定義

* WriteBatch が使える
* apply_batch が冪等
* snapshot 作成ができる
* snapshot load ができる
* wavelet inverted index が使える
* 1万枚規模で実用的 latency

## 30.3 v0.3 完了定義

* segment storage
* tombstone delete
* reader/writer separation
* Elixir Port integration stable
* worker reload
* protocol versioning

---

# 31. 将来拡張

## 31.1 Region Search

* image を tile に分割
* RegionSignature を作る
* region_id を持つ
* 結果に bounding box を返す

## 31.2 Local Feature

* ORB 風 binary descriptor
* keypoint
* descriptor matching
* RANSAC

## 31.3 mmap

* signatures.bin を固定長化
* postings を offset table 化
* reader を mmap 化

## 31.4 FFI / NIF

* query by signature のみ NIF 化候補
* 画像デコードや index build は NIF に入れない
* BEAM を落とさない設計を優先

---

# 32. 最終まとめ

`mrquery-rs` は、深層学習を使わない画像検索コアである。

Rust 側は以下に集中する。

* 画像署名
* インデックス
* クエリ
* versioned storage
* snapshot
* worker protocol

Elixir 側は以下に集中する。

* API
* 分散制御
* Raft
* supervision
* job management
* observability

Rust index は、Elixir から見ると deterministic な state machine である。

```text
IndexState(version N)
  + WriteBatch
  = IndexState(version N + 1)
```

query は常に特定の `IndexVersion` に対して実行される。

この設計により、画像検索エンジンと分散システムを混ぜずに済む。
Rust は検索性能とデータ構造に集中し、Elixir は運用と分散制御に集中する。

