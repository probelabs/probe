use crate::simd_ranking::SparseVector;
use ahash::AHashMap as HashMap;
use simsimd::SpatialSimilarity;

pub fn test_simd_implementation() {
    println!("Testing SIMD implementation...");

    // Test 1: Direct SimSIMD dot product
    let a = vec![1.0f32, 2.0, 3.0];
    let b = vec![4.0f32, 5.0, 6.0];
    let expected_dot = 1.0 * 4.0 + 2.0 * 5.0 + 3.0 * 6.0; // = 4 + 10 + 18 = 32

    println!("Testing direct SimSIMD dot product:");
    println!("a = {a:?}, b = {b:?}");
    println!("Expected dot product: {expected_dot}");

    if let Some(simd_dot) = f32::dot(&a, &b) {
        println!("SimSIMD dot product: {simd_dot}");
        println!(
            "SimSIMD is working: {}",
            (simd_dot - expected_dot).abs() < 0.001
        );
    } else {
        println!("SimSIMD dot product FAILED!");
    }

    // Test 2: Sparse vector operations
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

    println!("\nTesting sparse vectors:");
    println!(
        "Vector 1: indices={:?}, values={:?}",
        sparse1.indices, sparse1.values
    );
    println!(
        "Vector 2: indices={:?}, values={:?}",
        sparse2.indices, sparse2.values
    );

    // Test intersection
    let intersection = sparse1.intersect_indices(&sparse2);
    println!("Intersection: {intersection:?}");

    // Test dot product (should be 2*4 + 3*5 = 8 + 15 = 23)
    let dot_product = sparse1.dot_product(&sparse2);
    println!("Sparse SIMD dot product: {dot_product}");

    // Test manual calculation
    let manual_dot = sparse1.manual_dot_product(&sparse2);
    println!("Manual dot product: {manual_dot}");

    // Test optimized intersection
    let (vals1, vals2) = sparse1.intersect_with_values(&sparse2);
    println!("Intersected values: {vals1:?} • {vals2:?}");

    if let Some(direct_simd) = f32::dot(&vals1, &vals2) {
        println!("Direct SIMD on intersected values: {direct_simd}");
    }

    assert_eq!(intersection, vec![1, 2]);
    assert_eq!(manual_dot, 23.0);
    assert!(
        (dot_product - 23.0).abs() < 0.001,
        "SIMD dot product should be 23.0, got {dot_product}"
    );

    println!("SIMD test completed successfully!");
}
