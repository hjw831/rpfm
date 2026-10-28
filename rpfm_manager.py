
from flask import Flask, jsonify, render_template_string
from flask_cors import CORS
import subprocess
import psutil
import os
import signal
import json

app = Flask(__name__)
CORS(app)

# 설정
SERVER_PATH = r"C:\Users\jw900\VScodeProjects\rpfm\target\release\rpfm_server.exe"
SERVER_ARGS = ["--mcp-stdio"]
TEST_PACK = r"C:\GAME\TWmod\originals\tenzin_fps.pack"
LOG_PATH = r"C:\Users\jw900\VScodeProjects\rpfm\server_crash.log"

def get_server_process():
    for proc in psutil.process_iter(['pid', 'name']):
        try:
            if proc.info['name'] == "rpfm_server.exe":
                return proc
        except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.ZombieProcess):
            pass
    return None

@app.route('/status')
def status():
    proc = get_server_process()
    if proc:
        return jsonify({
            "status": "running",
            "pid": proc.pid,
            "message": "RPFM Server is active 🟢"
        })
    return jsonify({
        "status": "stopped",
        "message": "RPFM Server is offline 🔴"
    })

@app.route('/start')
def start():
    if get_server_process():
        return jsonify({"result": "error", "message": "Server is already running!"})

    try:
        # 로그 파일 생성 (기존 내용 삭제 후 새로 작성)
        with open(LOG_PATH, "w", encoding="utf-8") as f:
            f.write("--- Server Start Log ---\n")

        # 서버 실행 및 stdout/stderr를 파일로 연결
        # subprocess.DEVNULL 대신 파일 객체를 전달합니다.
        with open(LOG_PATH, "a", encoding="utf-8") as log_file:
            subprocess.Popen(
                [SERVER_PATH] + SERVER_ARGS,
                creationflags=subprocess.CREATE_NO_WINDOW,
                stdout=log_file,
                stderr=log_file,
                close_fds=True # 자식 프로세스가 부모의 파일 핸들을 물고 있지 않게 함
            )
        
        # 주의: Popen은 비동기이므로 여기서 바로 success를 반환하지만, 
        # 실제로는 서버가 뜨는 데 시간이 걸립니다.
        return jsonify({"result": "success", "message": "Server started. Checking stability... 🚀"})
    except Exception as e:
        return jsonify({"result": "error", "message": f"Failed to start: {str(e)}"})

@app.route('/stop')
def stop():
    proc = get_server_process()
    if not proc:
        return jsonify({"result": "error", "message": "Server is not running!"})

    try:
        proc.terminate()
        return jsonify({"result": "success", "message": "Server stopped successfully! 🛑"})
    except Exception as e:
        return jsonify({"result": "error", "message": f"Failed to stop: {str(e)}"})

@app.route('/test')
def test():
    try:
        subprocess.run(
            ["python", r"C:\Users\jw900\VScodeProjects\rpfm\mcp_test_script.py"],
            capture_output=True, text=True, timeout=15
        )
        result_path = r"C:\Users\jw900\VScodeProjects\rpfm\rpfm_result.txt"
        if os.path.exists(result_path):
            with open(result_path, "r", encoding="utf-8") as f:
                content = f.read()
            return jsonify({"result": "success", "data": content})
        else:
            return jsonify({"result": "error", "message": "Result file not found. Server might not be responding."})
    except Exception as e:
        return jsonify({"result": "error", "message": f"Test failed: {str(e)}"})

if __name__ == '__main__':
    print("RPFM Server Manager starting on http://127.0.0.1:5000")
    app.run(port=5000, debug=False)
