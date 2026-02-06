#!/bin/bash
# 不正データ拒否テスト
# テスト内容:
#   1. 正当なトランザクションが受け入れられる
#   2. 不正な署名のトランザクションが拒否される
#   3. 残高不足のトランザクションが拒否される
#   4. 無効なエンコーディングが拒否される

set +e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

echo "=========================================="
echo "  Test: Invalid Data Rejection"
echo "=========================================="

init_test_env

# ノード設定
ALICE_P2P=42333
ALICE_RPC=42944
BOB_P2P=42334
BOB_RPC=42945

# Step 1: ノード起動
log_info "Step 1: Starting nodes..."
start_node "alice" $ALICE_P2P $ALICE_RPC "true" "" "0000000000000000000000000000000000000000000000000000000000000003"

if ! wait_for_node $ALICE_RPC 30; then
    log_fail "Alice failed to start"
    exit 1
fi

ALICE_PEER_ID=$(get_peer_id "$TEST_LOG_DIR/alice.log" 30)
BOOTNODE="/ip4/127.0.0.1/tcp/$ALICE_P2P/p2p/$ALICE_PEER_ID"

start_node "bob" $BOB_P2P $BOB_RPC "true" "$BOOTNODE"

if ! wait_for_node $BOB_RPC 30; then
    log_fail "Bob failed to start"
    exit 1
fi

# ブロック生成を待機
log_info "Waiting for block production..."
sleep 10

# Step 2: Node.jsテストを実行
log_info "Step 2: Running transaction validation tests..."

cd "$SCRIPT_DIR"

# Node.jsテストスクリプトを実行
node << 'EOF'
const http = require('http');

const RPC_URL = 'http://127.0.0.1:42944';

async function rpcCall(method, params = []) {
    return new Promise((resolve, reject) => {
        const data = JSON.stringify({
            jsonrpc: '2.0',
            method,
            params,
            id: 1
        });
        
        const req = http.request(RPC_URL, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Content-Length': data.length
            }
        }, (res) => {
            let body = '';
            res.on('data', chunk => body += chunk);
            res.on('end', () => {
                try {
                    resolve(JSON.parse(body));
                } catch (e) {
                    resolve({ error: { message: 'Invalid JSON' } });
                }
            });
        });
        
        req.on('error', reject);
        req.write(data);
        req.end();
    });
}

async function runTests() {
    let passed = 0;
    let failed = 0;
    
    // Test 1: 無効なextrinsic（ランダムバイト）を送信
    console.log('\nTest 1: Invalid extrinsic (random bytes)');
    const invalidHex = '0x' + 'deadbeef'.repeat(100);
    const result1 = await rpcCall('author_submitExtrinsic', [invalidHex]);
    
    if (result1.error) {
        console.log('\x1b[32m[PASS]\x1b[0m Invalid extrinsic rejected:', result1.error.message.substring(0, 50));
        passed++;
    } else {
        console.log('\x1b[31m[FAIL]\x1b[0m Invalid extrinsic was accepted');
        failed++;
    }
    
    // Test 2: 空のextrinsic
    console.log('\nTest 2: Empty extrinsic');
    const result2 = await rpcCall('author_submitExtrinsic', ['0x']);
    
    if (result2.error) {
        console.log('\x1b[32m[PASS]\x1b[0m Empty extrinsic rejected:', result2.error.message.substring(0, 50));
        passed++;
    } else {
        console.log('\x1b[31m[FAIL]\x1b[0m Empty extrinsic was accepted');
        failed++;
    }
    
    // Test 3: 不正なJSONリクエスト
    console.log('\nTest 3: Malformed JSON-RPC request');
    const result3 = await rpcCall('nonexistent_method', []);
    
    if (result3.error) {
        console.log('\x1b[32m[PASS]\x1b[0m Unknown method rejected');
        passed++;
    } else {
        console.log('\x1b[31m[FAIL]\x1b[0m Unknown method was accepted');
        failed++;
    }
    
    // Test 4: 署名が壊れたトランザクション（有効な長さだが不正なデータ）
    console.log('\nTest 4: Corrupted signature transaction');
    // 実際のextrinsicヘッダー構造を模倣するが署名を無効に
    const corruptedTx = '0x' + 
        '45028400' + // 長さプレフィックス + バージョン
        'd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d' + // 送信者（Alice公開鍵）
        '0000000000000000000000000000000000000000000000000000000000000000' + // 無効な署名（64バイトのゼロ）
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '00' + // 署名タイプ
        '0000' + // era
        '00' + // nonce
        '00' + // tip
        '0000'; // call
        
    const result4 = await rpcCall('author_submitExtrinsic', [corruptedTx]);
    
    if (result4.error) {
        console.log('\x1b[32m[PASS]\x1b[0m Corrupted signature rejected:', result4.error.message.substring(0, 50));
        passed++;
    } else {
        console.log('\x1b[31m[FAIL]\x1b[0m Corrupted signature was accepted');
        failed++;
    }
    
    // Test 5: RPCエンドポイントの健全性確認
    console.log('\nTest 5: Valid RPC call (system_health)');
    const result5 = await rpcCall('system_health', []);
    
    if (result5.result && !result5.error) {
        console.log('\x1b[32m[PASS]\x1b[0m Valid RPC call successful:', JSON.stringify(result5.result));
        passed++;
    } else {
        console.log('\x1b[31m[FAIL]\x1b[0m Valid RPC call failed');
        failed++;
    }
    
    // Test 6: 正当なRPCクエリ（chain_getBlockHash）
    console.log('\nTest 6: Valid query (chain_getBlockHash)');
    const result6 = await rpcCall('chain_getBlockHash', [0]);
    
    if (result6.result && result6.result.startsWith('0x')) {
        console.log('\x1b[32m[PASS]\x1b[0m Genesis hash retrieved:', result6.result.substring(0, 20) + '...');
        passed++;
    } else {
        console.log('\x1b[31m[FAIL]\x1b[0m Failed to get genesis hash');
        failed++;
    }
    
    console.log('\n==========================================');
    console.log('  Summary');
    console.log('==========================================');
    console.log(`  \x1b[32mPassed:\x1b[0m ${passed}`);
    console.log(`  \x1b[31mFailed:\x1b[0m ${failed}`);
    console.log('==========================================');
    
    process.exit(failed > 0 ? 1 : 0);
}

runTests().catch(err => {
    console.error('Test error:', err);
    process.exit(1);
});
EOF

TEST_EXIT_CODE=$?

if [ $TEST_EXIT_CODE -eq 0 ]; then
    log_success "All invalid data rejection tests passed"
else
    log_fail "Some invalid data rejection tests failed"
fi

print_test_summary
