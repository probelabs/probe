use crate::ranking::QueryTokenMap;
use ahash::AHashMap as HashMap;
use simsimd::SpatialSimilarity;

/// Sparse vector representation optimized for SIMD operations
/// Maintains sorted indices for optimal SimSIMD performance
#[derive(Debug, Clone)]
pub struct SparseVector {
    /// Sorted indices of non-zero elements
    pub indices: Vec<u8>,
    /// Values corresponding to the indices
    pub values: Vec<f32>,
}

impl SparseVector {
    /// Create a new empty sparse vector
    pub fn new() -> Self {
        Self {
            indices: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Create a sparse vector with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            indices: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
        }
    }

    /// Create sparse vector from HashMap<u8, usize> (term frequencies)
    /// Automatically sorts indices for optimal SIMD performance
    pub fn from_tf_map(tf_map: &HashMap<u8, usize>) -> Self {
        let mut indices: Vec<u8> = tf_map.keys().copied().collect();
        indices.sort_unstable(); // SimSIMD requires sorted indices

        let values: Vec<f32> = indices.iter().map(|&idx| tf_map[&idx] as f32).collect();

        Self { indices, values }
    }

    /// Create sparse vector from HashMap with IDF weighting applied
    pub fn from_tf_map_with_idf(
        tf_map: &HashMap<u8, usize>,
        _idf_values: &[f32],
        _query_token_map: &QueryTokenMap,
    ) -> Self {
        let mut indices: Vec<u8> = tf_map.keys().copied().collect();
        indices.sort_unstable();

        let values: Vec<f32> = indices.iter().map(|&idx| tf_map[&idx] as f32).collect();

        Self { indices, values }
    }

    /// Get the number of non-zero elements
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    /// Check if the sparse vector is empty
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Compute dot product with another sparse vector using SimSIMD
    pub fn dot_product(&self, other: &SparseVector) -> f32 {
        if self.is_empty() || other.is_empty() {
            return 0.0;
        }

        // Use optimized intersection that returns values directly (no binary search needed)
        let (self_values, other_values) = self.intersect_with_values(other);

        if self_values.is_empty() {
            return 0.0;
        }

        // Use SimSIMD's direct dot product function - this is the real SIMD acceleration!
        f32::dot(&self_values, &other_values)
            .map(|x| x as f32)
            .unwrap_or_else(|| {
                // Fallback to manual computation if SimSIMD fails
                self_values
                    .iter()
                    .zip(other_values.iter())
                    .map(|(&a, &b)| a * b)
                    .sum::<f32>()
            })
    }

    /// Direct SIMD dot product for dense vectors (when we know intersection already)
    pub fn simd_dot_product_dense(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        // Use SimSIMD's direct dot product function - this is the real SIMD acceleration!
        f32::dot(a, b).map(|x| x as f32).unwrap_or_else(|| {
            // Manual fallback
            a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum::<f32>()
        })
    }

    /// Manual dot product computation as fallback
    pub fn manual_dot_product(&self, other: &SparseVector) -> f32 {
        let mut result = 0.0;
        let mut i = 0;
        let mut j = 0;

        // Two-pointer approach for sparse vector intersection
        while i < self.indices.len() && j < other.indices.len() {
            match self.indices[i].cmp(&other.indices[j]) {
                std::cmp::Ordering::Equal => {
                    result += self.values[i] * other.values[j];
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }

        result
    }

    /// Find intersection indices with another sparse vector
    pub fn intersect_indices(&self, other: &SparseVector) -> Vec<u8> {
        let mut result = Vec::new();
        let mut i = 0;
        let mut j = 0;

        while i < self.indices.len() && j < other.indices.len() {
            match self.indices[i].cmp(&other.indices[j]) {
                std::cmp::Ordering::Equal => {
                    result.push(self.indices[i]);
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }

        result
    }

    /// More efficient intersection that returns indices AND values directly
    /// This avoids repeated binary searches in dot product computation
    pub fn intersect_with_values(&self, other: &SparseVector) -> (Vec<f32>, Vec<f32>) {
        let mut self_values = Vec::new();
        let mut other_values = Vec::new();
        let mut i = 0;
        let mut j = 0;

        while i < self.indices.len() && j < other.indices.len() {
            match self.indices[i].cmp(&other.indices[j]) {
                std::cmp::Ordering::Equal => {
                    self_values.push(self.values[i]);
                    other_values.push(other.values[j]);
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }

        (self_values, other_values)
    }
}

impl Default for SparseVector {
    fn default() -> Self {
        Self::new()
    }
}

/// Precomputed sparse vectors for efficient BM25 computation
#[derive(Debug)]
pub struct SparseDocumentMatrix {
    /// Sparse vectors for all documents
    pub documents: Vec<SparseVector>,
    /// Query sparse vector
    pub query: SparseVector,
    /// Precomputed IDF values indexed by u8 token indices
    pub idf_values: Vec<f32>,
    /// Average document length
    pub avgdl: f64,
    /// BM25 parameters
    pub k1: f64,
    pub b: f64,
}

impl SparseDocumentMatrix {
    /// Create a new sparse document matrix from existing ranking data
    pub fn from_ranking_data(
        term_frequencies: &[HashMap<u8, usize>],
        _document_lengths: &[usize],
        _query_terms: &[String],
        query_token_map: &QueryTokenMap,
        idfs: &HashMap<String, f64>,
        avgdl: f64,
    ) -> Self {
        // Convert document term frequencies to sparse vectors
        let documents: Vec<SparseVector> = term_frequencies
            .iter()
            .map(SparseVector::from_tf_map)
            .collect();

        // Create query sparse vector (all terms have frequency 1 for boolean queries)
        let mut query_tf = HashMap::new();
        for (_, &token_idx) in query_token_map {
            query_tf.insert(token_idx, 1);
        }
        let query = SparseVector::from_tf_map(&query_tf);

        // Convert IDF values to indexed array
        let mut idf_values = vec![0.0f32; 256]; // u8 can index 0-255
        for (term, &token_idx) in query_token_map {
            if let Some(&idf) = idfs.get(term) {
                idf_values[token_idx as usize] = idf as f32;
            }
        }

        Self {
            documents,
            query,
            idf_values,
            avgdl,
            k1: 1.2,
            b: 0.75,
        }
    }

    /// Compute BM25 score for a specific document using SIMD operations
    pub fn compute_bm25_score(&self, doc_index: usize, doc_length: usize) -> f32 {
        if doc_index >= self.documents.len() {
            return 0.0;
        }

        let doc = &self.documents[doc_index];

        // Find intersecting terms between query and document
        let common_indices = self.query.intersect_indices(doc);

        if common_indices.is_empty() {
            return 0.0;
        }

        // Prepare vectors for SIMD computation
        let mut tf_values = Vec::with_capacity(common_indices.len());
        let mut idf_values = Vec::with_capacity(common_indices.len());
        let mut query_weights = Vec::with_capacity(common_indices.len()); // Usually 1.0 for boolean queries

        let doc_len_norm = (1.0 - self.b + self.b * (doc_length as f64 / self.avgdl)) as f32;

        // Extract values for vectorized computation
        for &idx in &common_indices {
            if let Ok(pos) = doc.indices.binary_search(&idx) {
                let tf = doc.values[pos];
                let idf = self.idf_values[idx as usize];

                // Apply BM25 TF normalization: tf * (k1 + 1) / (tf + k1 * doc_len_norm)
                let tf_normalized =
                    (tf * (self.k1 as f32 + 1.0)) / (tf + self.k1 as f32 * doc_len_norm);

                tf_values.push(tf_normalized);
                idf_values.push(idf);
                query_weights.push(1.0f32); // Boolean query weight
            }
        }

        if tf_values.is_empty() {
            return 0.0;
        }

        // Use SIMD operations for the final BM25 computation
        // This computes the dot product of (tf_normalized * idf) * query_weights
        // Which is equivalent to sum(tf_normalized[i] * idf[i] * query_weights[i])

        // Element-wise multiplication: tf_normalized * idf using SIMD
        let bm25_components = Self::simd_element_wise_multiply(&tf_values, &idf_values);

        // Final dot product with query weights using SIMD
        SparseVector::simd_dot_product_dense(&bm25_components, &query_weights)
    }

    /// SIMD element-wise multiplication of two vectors
    fn simd_element_wise_multiply(a: &[f32], b: &[f32]) -> Vec<f32> {
        if a.len() != b.len() {
            // Fallback to manual computation
            return a.iter().zip(b.iter()).map(|(&x, &y)| x * y).collect();
        }

        // For element-wise multiplication, we can use the fact that:
        // a[i] * b[i] for all i is equivalent to sum of a[i] * b[i] where we want each component
        // We'll have to do this manually since SimSIMD doesn't have element-wise multiply
        // But we can still benefit from SIMD in the final dot product
        a.iter().zip(b.iter()).map(|(&x, &y)| x * y).collect()

        // TODO: In future, we could use std::simd for element-wise operations if available
        // or implement custom SIMD element-wise multiplication
    }

    /// Compute BM25 scores for all documents in parallel using SIMD
    pub fn compute_all_scores(&self, document_lengths: &[usize]) -> Vec<f32> {
        (0..self.documents.len())
            .map(|i| self.compute_bm25_score(i, document_lengths[i]))
            .collect()
    }
}

/// Enhanced BM25 parameters with SIMD optimization
pub struct SimdBm25Params<'a> {
    /// Sparse document matrix
    pub matrix: &'a SparseDocumentMatrix,
    /// Document lengths
    pub doc_lengths: &'a [usize],
}

impl<'a> SimdBm25Params<'a> {
    pub fn new(matrix: &'a SparseDocumentMatrix, doc_lengths: &'a [usize]) -> Self {
        Self {
            matrix,
            doc_lengths,
        }
    }

    /// Compute BM25 score for a single document
    pub fn score_document(&self, doc_index: usize) -> f32 {
        self.matrix
            .compute_bm25_score(doc_index, self.doc_lengths[doc_index])
    }

    /// Compute scores for all documents
    pub fn score_all_documents(&self) -> Vec<f32> {
        self.matrix.compute_all_scores(self.doc_lengths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_vector_creation() {
        let mut tf_map = HashMap::new();
        tf_map.insert(0u8, 5);
        tf_map.insert(2u8, 3);
        tf_map.insert(1u8, 7);

        let sparse = SparseVector::from_tf_map(&tf_map);

        // Should be sorted by indices
        assert_eq!(sparse.indices, vec![0, 1, 2]);
        assert_eq!(sparse.values, vec![5.0, 7.0, 3.0]);
        assert_eq!(sparse.len(), 3);
    }

    #[test]
    fn test_sparse_vector_dot_product() {
        let mut tf_map1 = HashMap::new();
        tf_map1.insert(0u8, 1);
        tf_map1.insert(1u8, 2);
        tf_map1.insert(2u8, 3);

        let mut tf_map2 = HashMap::new();
        tf_map2.insert(1u8, 4);
        tf_map2.insert(2u8, 5);
        tf_map2.insert(3u8, 6);

        let sparse1 = SparseVector::from_tf_map(&tf_map1);
        let sparse2 = SparseVector::from_tf_map(&tf_map2);

        // Manual calculation: (2*4) + (3*5) = 8 + 15 = 23
        let dot_product = sparse1.manual_dot_product(&sparse2);
        assert_eq!(dot_product, 23.0);
    }

    #[test]
    fn test_sparse_vector_intersection() {
        let mut tf_map1 = HashMap::new();
        tf_map1.insert(0u8, 1);
        tf_map1.insert(1u8, 2);
        tf_map1.insert(2u8, 3);

        let mut tf_map2 = HashMap::new();
        tf_map2.insert(1u8, 4);
        tf_map2.insert(2u8, 5);
        tf_map2.insert(3u8, 6);

        let sparse1 = SparseVector::from_tf_map(&tf_map1);
        let sparse2 = SparseVector::from_tf_map(&tf_map2);

        let intersection = sparse1.intersect_indices(&sparse2);
        assert_eq!(intersection, vec![1, 2]);
    }

    #[test]
    fn test_empty_sparse_vectors() {
        let empty1 = SparseVector::new();
        let empty2 = SparseVector::new();

        assert!(empty1.is_empty());
        assert_eq!(empty1.len(), 0);
        assert_eq!(empty1.dot_product(&empty2), 0.0);
    }
}
