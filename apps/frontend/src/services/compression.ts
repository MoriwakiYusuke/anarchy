/**
 * Compression Service
 *
 * gzip圧縮/解凍。ブラウザのCompressionStream APIを使用。
 *
 * @module services/compression
 */

/**
 * データをgzip圧縮
 *
 * @param data - 圧縮するデータ
 * @returns 圧縮されたデータ（または元データが小さい場合はそのまま）と圧縮フラグ
 */
export async function compress(
  data: Uint8Array
): Promise<{ data: Uint8Array; compressed: boolean }> {
  // Skip compression for small data
  if (data.length < 256) {
    return { data, compressed: false };
  }

  try {
    const stream = new Blob([data as BlobPart]).stream();
    const compressedStream = stream.pipeThrough(
      new CompressionStream('gzip')
    );

    const compressedBlob = await new Response(compressedStream).blob();
    const compressedData = new Uint8Array(await compressedBlob.arrayBuffer());

    // Only use compression if it actually reduced size
    if (compressedData.length < data.length) {
      return { data: compressedData, compressed: true };
    }

    return { data, compressed: false };
  } catch {
    // Fallback: return uncompressed
    return { data, compressed: false };
  }
}

/**
 * gzip解凍
 *
 * @param data - 圧縮されたデータ
 * @returns 解凍されたデータ
 */
export async function decompress(data: Uint8Array): Promise<Uint8Array> {
  try {
    const stream = new Blob([data as BlobPart]).stream();
    const decompressedStream = stream.pipeThrough(
      new DecompressionStream('gzip')
    );

    const decompressedBlob = await new Response(decompressedStream).blob();
    return new Uint8Array(await decompressedBlob.arrayBuffer());
  } catch (error) {
    throw new Error(`Decompression failed: ${error}`);
  }
}

/**
 * データが圧縮されているかチェック（gzipマジックナンバー）
 *
 * @param data - チェックするデータ
 * @returns gzip圧縮されていればtrue
 */
export function isGzipped(data: Uint8Array): boolean {
  return data.length >= 2 && data[0] === 0x1f && data[1] === 0x8b;
}
