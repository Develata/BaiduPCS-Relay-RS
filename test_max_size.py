#!/usr/bin/env python3
"""测试 MAX_ZIP_SIZE 和分卷功能"""
import requests
import json
import sys

# 配置
BASE_URL = "http://localhost:5200"
ACCESS_TOKEN = "change-me"

# 测试的 fsid（陶哲轩教你学数学.pdf 的文件夹）
FOLDER_FSID = 433454007834933  # test/ 目录的 fsid

def test_zip_with_size_limit():
    """测试 ZIP 大小限制和分卷功能"""
    print(f"\n{'='*60}")
    print("测试: MAX_ZIP_SIZE 限制和分卷功能")
    print(f"{'='*60}\n")
    
    print(f"📁 请求打包 fsid={FOLDER_FSID} (test/ 目录，包含 7.9MB PDF)")
    print(f"⚙️  当前 MAX_ZIP_SIZE=10485760 (10MB)")
    print(f"💡 预期: 文件大小 < 10MB，应该返回单个 ZIP\n")
    
    # 构造请求
    payload = {
        "fsids": [FOLDER_FSID],
        "archive_name": "test_folder",
        "token": ACCESS_TOKEN
    }
    
    # 发送请求
    print(f"🌐 POST {BASE_URL}/api/zip")
    print(f"   payload: {json.dumps(payload, indent=2)}\n")
    
    try:
        resp = requests.post(f"{BASE_URL}/api/zip", json=payload, timeout=60)
        print(f"📊 HTTP Status: {resp.status_code}")
        print(f"📊 Content-Type: {resp.headers.get('Content-Type', 'N/A')}")
        print(f"📊 Content-Length: {len(resp.content)} bytes ({len(resp.content)/1024/1024:.2f} MB)")
        
        if resp.status_code == 200:
            content_type = resp.headers.get('Content-Type', '')
            
            if 'application/json' in content_type:
                # 返回了 JSON（多分卷）
                data = resp.json()
                print(f"\n✅ 返回分卷信息:")
                print(json.dumps(data, indent=2, ensure_ascii=False))
                
                if data.get('success'):
                    print(f"\n📦 总共 {data['total_parts']} 个 part")
                    print(f"📦 总大小: {data['total_size']/1024/1024:.2f} MB")
                    for part in data.get('parts', []):
                        print(f"   - Part {part['part_num']}: {part['filename']} ({part['size_bytes']/1024/1024:.2f} MB)")
            
            elif 'application/zip' in content_type:
                # 返回了单个 ZIP 文件
                filename = resp.headers.get('Content-Disposition', '').split('filename=')[-1].strip('"')
                print(f"\n✅ 返回单个 ZIP 文件:")
                print(f"   文件名: {filename}")
                print(f"   大小: {len(resp.content)/1024/1024:.2f} MB")
                
                # 保存文件
                output_file = '/tmp/test_max_size.zip'
                with open(output_file, 'wb') as f:
                    f.write(resp.content)
                print(f"   已保存到: {output_file}")
                
                # 验证 ZIP 文件
                import zipfile
                try:
                    with zipfile.ZipFile(output_file, 'r') as zf:
                        print(f"\n📝 ZIP 内容:")
                        for info in zf.infolist():
                            print(f"   - {info.filename} ({info.file_size} bytes)")
                except Exception as e:
                    print(f"\n❌ ZIP 验证失败: {e}")
        else:
            print(f"\n❌ 请求失败: {resp.text}")
            
    except Exception as e:
        print(f"\n❌ 请求出错: {e}")
        import traceback
        traceback.print_exc()

def test_with_small_limit():
    """测试非常小的限制，强制分卷"""
    print(f"\n{'='*60}")
    print("提示: 如需测试分卷功能，请重启服务器并设置 MAX_ZIP_SIZE=5242880 (5MB)")
    print("命令: docker exec -d rust-manual-run bash -c 'cd /app && pkill baidu-web-server; sleep 1; MAX_ZIP_SIZE=5242880 ./target/release/baidu-web-server'")
    print(f"{'='*60}\n")

if __name__ == "__main__":
    test_zip_with_size_limit()
    test_with_small_limit()
