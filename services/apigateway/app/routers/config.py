"""
配置导出API路由
处理系统配置的导出功能
"""

import logging
import os
import re
import zipfile
import tempfile
import shutil
import subprocess
import asyncio
import stat
import json
from datetime import datetime
from typing import Dict, Any, List, Optional
from pathlib import Path
from fastapi import APIRouter, HTTPException, status, UploadFile, File, Body
from fastapi.responses import FileResponse
from pydantic import BaseModel

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/config", tags=["配置管理"])

# test 124
# 升级相关目录
# 注意：必须使用宿主机可访问的路径，因为升级容器在宿主机上运行
# 使用 /opt/MonarchEdge/upgrade（通过 docker-compose.yml 挂载）
UPGRADE_DIR = Path("/opt/MonarchEdge/upgrade")
UPGRADE_LOG_FILE = UPGRADE_DIR / "upgrade.log"
UPGRADE_STATUS_FILE = UPGRADE_DIR / "upgrade_status.json"

# Docker Socket 路径（需要在 docker-compose.yml 中挂载）
DOCKER_SOCKET = Path("/var/run/docker.sock")

# 上传中断标志（全局状态）
_upload_abort_flag = False


def _ensure_upgrade_dir():
    """确保升级目录存在"""
    try:
        UPGRADE_DIR.mkdir(parents=True, exist_ok=True)
        return True
    except PermissionError:
        # 如果无权创建，尝试在挂载的 /opt/MonarchEdge 中创建
        # 这种情况下目录应该已经由宿主机创建好了
        return UPGRADE_DIR.exists()
    except Exception as e:
        logger.error(f"创建升级目录异常: {e}")
        return False


def _check_upgrade_dir_available():
    """检查升级目录是否可用"""
    if not UPGRADE_DIR.exists():
        return False, f"升级目录不存在: {UPGRADE_DIR}（需要在宿主机上创建: sudo mkdir -p {UPGRADE_DIR} && sudo chmod 777 {UPGRADE_DIR}）"
    
    if not os.access(UPGRADE_DIR, os.W_OK):
        return False, f"升级目录无写入权限（需要在宿主机上执行: sudo chmod 777 {UPGRADE_DIR}）"
    
    return True, None


@router.get("/export", response_class=FileResponse)
async def export_config():
    """
    导出配置文件
    
    从 /opt/MonarchEdge/data 目录导出配置文件
    如果目录不存在，则返回失败消息
    """
    try:
        config_dir = Path("/opt/MonarchEdge/data")
        
        # 检查目录是否存在
        if not config_dir.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail={
                    "success": False,
                    "message": "配置目录不存在",
                    "path": str(config_dir)
                }
            )
        
        # 检查目录是否为空
        if not any(config_dir.iterdir()):
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail={
                    "success": False,
                    "message": "配置目录为空",
                    "path": str(config_dir)
                }
            )
        
        # 创建临时zip文件
        temp_file = tempfile.NamedTemporaryFile(
            delete=False, 
            suffix='.zip', 
            prefix='config_export_'
        )
        temp_file.close()
        
        try:
            # 压缩配置目录
            with zipfile.ZipFile(temp_file.name, 'w', zipfile.ZIP_DEFLATED) as zipf:
                # 遍历目录中的所有文件
                for root, dirs, files in os.walk(config_dir):
                    for file in files:
                        file_path = Path(root) / file
                        # 计算相对路径
                        arcname = file_path.relative_to(config_dir)
                        zipf.write(file_path, arcname)
                        logger.info(f"添加文件到压缩包: {arcname}")
            
            # 检查生成的zip文件大小
            zip_size = os.path.getsize(temp_file.name)
            logger.info(f"配置导出成功，压缩包大小: {zip_size} 字节")
            
            # 返回文件
            return FileResponse(
                path=temp_file.name,
                filename="monarchedge_config_export.zip",
                media_type="application/zip",
                background=None  # 不在后台删除，让操作系统处理临时文件
            )
            
        except Exception as e:
            # 如果出错，删除临时文件
            if os.path.exists(temp_file.name):
                os.unlink(temp_file.name)
            raise
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"导出配置异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={
                "success": False,
                "message": f"导出配置失败: {str(e)}"
            }
        )


@router.get("/check", response_model=Dict[str, Any])
async def check_config_dir():
    """
    检查配置目录状态
    
    检查 /opt/MonarchEdge/data 目录是否存在以及其中的文件数量
    """
    try:
        config_dir = Path("/opt/MonarchEdge/data")
        
        exists = config_dir.exists()
        is_dir = config_dir.is_dir() if exists else False
        
        file_count = 0
        total_size = 0
        
        if exists and is_dir:
            try:
                # 统计文件数量和总大小
                for root, dirs, files in os.walk(config_dir):
                    file_count += len(files)
                    for file in files:
                        file_path = Path(root) / file
                        try:
                            total_size += file_path.stat().st_size
                        except Exception as e:
                            logger.warning(f"无法获取文件大小: {file_path}, 错误: {e}")
            except Exception as e:
                logger.error(f"遍历配置目录异常: {e}")
        
        return {
            "success": True,
            "message": "配置目录检查完成",
            "data": {
                "path": str(config_dir),
                "exists": exists,
                "is_directory": is_dir,
                "file_count": file_count,
                "total_size_bytes": total_size,
                "total_size_mb": round(total_size / (1024 * 1024), 2)
            }
        }
        
    except Exception as e:
        logger.error(f"检查配置目录异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={
                "success": False,
                "message": f"检查配置目录失败: {str(e)}"
            }
        )


@router.post("/import", response_model=Dict[str, Any])
async def import_config(file: UploadFile = File(...)):
    """
    导入配置文件
    
    上传ZIP压缩包，解压到 /opt/MonarchEdge/data 目录
    存在同名文件则覆盖
    """
    config_dir = Path("/opt/MonarchEdge/data")
    temp_zip_path = None
    temp_extract_dir = None
    
    try:
        # 验证文件类型
        if not file.filename.endswith('.zip'):
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "success": False,
                    "message": "只支持ZIP格式的压缩文件"
                }
            )
        
        # 验证文件大小（限制为100MB）
        max_size = 100 * 1024 * 1024  # 100MB
        file_content = await file.read()
        file_size = len(file_content)
        
        if file_size == 0:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "success": False,
                    "message": "上传的文件为空"
                }
            )
        
        if file_size > max_size:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "success": False,
                    "message": f"文件大小超过限制（最大100MB），当前文件大小: {round(file_size / (1024 * 1024), 2)}MB"
                }
            )
        
        logger.info(f"开始导入配置，文件名: {file.filename}, 大小: {round(file_size / (1024 * 1024), 2)}MB")
        
        # 创建临时文件保存上传的ZIP
        with tempfile.NamedTemporaryFile(delete=False, suffix='.zip', prefix='config_import_') as temp_zip:
            temp_zip.write(file_content)
            temp_zip_path = temp_zip.name
        
        # 验证ZIP文件完整性
        try:
            with zipfile.ZipFile(temp_zip_path, 'r') as zip_ref:
                # 测试ZIP文件完整性
                bad_file = zip_ref.testzip()
                if bad_file:
                    raise Exception(f"ZIP文件损坏，错误文件: {bad_file}")
                
                # 获取文件列表
                file_list = zip_ref.namelist()
                if not file_list:
                    raise Exception("ZIP文件中没有文件")
                
                logger.info(f"ZIP文件验证通过，包含 {len(file_list)} 个文件")
        except zipfile.BadZipFile:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "success": False,
                    "message": "无效的ZIP文件格式"
                }
            )
        except Exception as e:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "success": False,
                    "message": f"ZIP文件验证失败: {str(e)}"
                }
            )
        
        # 创建临时解压目录
        temp_extract_dir = tempfile.mkdtemp(prefix='config_extract_')
        
        # 解压到临时目录
        extracted_files: List[str] = []
        with zipfile.ZipFile(temp_zip_path, 'r') as zip_ref:
            for file_info in zip_ref.filelist:
                # 跳过目录
                if file_info.is_dir():
                    continue
                
                # 安全检查：防止路径遍历攻击
                file_path = Path(file_info.filename)
                if file_path.is_absolute() or '..' in file_path.parts:
                    logger.warning(f"跳过不安全的文件路径: {file_info.filename}")
                    continue
                
                # 解压文件
                zip_ref.extract(file_info, temp_extract_dir)
                extracted_files.append(file_info.filename)
                logger.info(f"解压文件: {file_info.filename}")
        
        if not extracted_files:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "success": False,
                    "message": "ZIP文件中没有有效的文件"
                }
            )
        
        # 确保目标目录存在
        config_dir.mkdir(parents=True, exist_ok=True)
        
        # 复制文件到目标目录（覆盖已存在的文件）
        imported_files: List[str] = []
        overwritten_files: List[str] = []
        new_files: List[str] = []
        
        for rel_path in extracted_files:
            src_file = Path(temp_extract_dir) / rel_path
            dest_file = config_dir / rel_path
            
            # 创建目标文件的父目录
            dest_file.parent.mkdir(parents=True, exist_ok=True)
            
            # 检查文件是否已存在
            existed = dest_file.exists()
            
            # 复制文件（三级降级策略）
            # 1. copy2: 复制内容 + 元数据（时间戳、权限）
            # 2. copy:  复制内容 + 权限
            # 3. copyfile: 仅复制内容（不管权限和元数据）
            try:
                shutil.copy2(src_file, dest_file)
            except (PermissionError, OSError) as e1:
                logger.warning(f"无法保留文件元数据 {rel_path}，尝试降级: {e1}")
                try:
                    shutil.copy(src_file, dest_file)
                except (PermissionError, OSError) as e2:
                    logger.warning(f"无法复制文件权限 {rel_path}，使用基础复制: {e2}")
                    # 最基础的复制：仅复制文件内容，不管任何元数据和权限
                    shutil.copyfile(src_file, dest_file)
            
            imported_files.append(rel_path)
            if existed:
                overwritten_files.append(rel_path)
                logger.info(f"覆盖文件: {rel_path}")
            else:
                new_files.append(rel_path)
                logger.info(f"新增文件: {rel_path}")
        
        logger.info(f"配置导入成功，共 {len(imported_files)} 个文件（新增: {len(new_files)}, 覆盖: {len(overwritten_files)}）")
        
        return {
            "success": True,
            "message": "配置导入成功",
            "data": {
                "total_files": len(imported_files),
                "new_files": len(new_files),
                "overwritten_files": len(overwritten_files),
                "target_directory": str(config_dir),
                "files": {
                    "new": new_files,
                    "overwritten": overwritten_files
                }
            }
        }
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"导入配置异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={
                "success": False,
                "message": f"导入配置失败: {str(e)}"
            }
        )
    finally:
        # 清理临时文件
        if temp_zip_path and os.path.exists(temp_zip_path):
            try:
                os.unlink(temp_zip_path)
                logger.info(f"已删除临时ZIP文件: {temp_zip_path}")
            except Exception as e:
                logger.warning(f"删除临时ZIP文件失败: {e}")
        
        if temp_extract_dir and os.path.exists(temp_extract_dir):
            try:
                shutil.rmtree(temp_extract_dir)
                logger.info(f"已删除临时解压目录: {temp_extract_dir}")
            except Exception as e:
                logger.warning(f"删除临时解压目录失败: {e}")


def _check_docker_socket() -> bool:
    """检查 Docker Socket 是否可用"""
    return DOCKER_SOCKET.exists() and os.access(DOCKER_SOCKET, os.R_OK | os.W_OK)


def _read_upgrade_status() -> Dict[str, Any]:
    """读取升级状态文件"""
    if not UPGRADE_STATUS_FILE.exists():
        return {"status": "idle"}  # 返回空闲状态
    
    try:
        with open(UPGRADE_STATUS_FILE, 'r', encoding='utf-8') as f:
            content = f.read()
            
            # 清理可能存在的 ANSI 控制字符和其他非法字符
            # ANSI 控制字符模式: \x1b[...m
            # 移除 ANSI 转义序列
            content = re.sub(r'\x1b\[[0-9;]*m', '', content)
            # 移除其他控制字符（保留换行、制表符）
            content = re.sub(r'[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]', '', content)
            
            # 尝试解析 JSON
            return json.loads(content)
    except json.JSONDecodeError as e:
        logger.warning(f"读取升级状态文件失败（JSON 解析错误）: {e}")
        # 返回原始内容的前 100 个字符，方便调试
        try:
            with open(UPGRADE_STATUS_FILE, 'r', encoding='utf-8') as f:
                preview = f.read(100)
            logger.warning(f"状态文件内容预览: {repr(preview)}")
        except:
            pass
        return {"status": "idle"}  # 返回空闲状态
    except Exception as e:
        logger.warning(f"读取升级状态文件失败: {e}")
        return {"status": "idle"}  # 返回空闲状态


def _write_upgrade_status(status_data: Dict[str, Any]):
    """写入升级状态文件"""
    try:
        with open(UPGRADE_STATUS_FILE, 'w') as f:
            json.dump(status_data, f, indent=2, ensure_ascii=False)
    except Exception as e:
        logger.error(f"写入升级状态文件失败: {e}")


@router.post("/upgrade", response_model=Dict[str, Any])
async def upload_and_run_upgrade(
    file: UploadFile = File(...),
):
    """
    上传升级包并自动运行
    
    上传 .run 文件（如 MonarchEdge-arm64-20260123-alarmsrv-apigateway.run），
    在宿主机上执行升级脚本（install.sh --auto）
    
    参数:
    - file: .run 格式的升级包文件
    
    工作流程:
    1. 保存 .run 文件到持久化目录 /opt/MonarchEdge/upgrade/
    2. 通过 Docker Socket 在宿主机上执行升级脚本
    3. 升级脚本会自动：
       - 解压升级包
       - 加载新的 Docker 镜像
       - 智能检测镜像变化
       - 重启变更的容器（包括本容器）
    
    要求:
    1. 必须挂载 Docker Socket: /var/run/docker.sock:/var/run/docker.sock
    2. 必须挂载持久化目录: /opt/MonarchEdge（volume 或 bind mount）
    3. 升级过程中 API 容器可能会被重启，这是正常现象
    
    返回:
    - success: 是否成功启动升级
    - message: 状态消息
    - data: 包含升级任务信息（文件名、大小、日志路径等）
    """
    try:
        # 确保升级目录存在
        _ensure_upgrade_dir()
        
        # 检查升级目录是否可用
        available, error_msg = _check_upgrade_dir_available()
        if not available:
            raise HTTPException(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                detail={
                    "success": False,
                    "message": f"Upgrade feature unavailable: {error_msg}",
                    "hint": "请在宿主机上执行: sudo mkdir -p /opt/MonarchEdge/upgrade && sudo chown -R $(id -u):docker /opt/MonarchEdge/upgrade && sudo chmod 775 /opt/MonarchEdge/upgrade",
                    "fix_script": "./scripts/fix-upgrade-permissions.sh"
                }
            )
        
        # 检查 Docker Socket 是否可用
        if not _check_docker_socket():
            raise HTTPException(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                detail={
                    "success": False,
                    "message": "Docker Socket unavailable, please ensure /var/run/docker.sock is mounted in docker-compose.yml",
                    "hint": "volumes: - /var/run/docker.sock:/var/run/docker.sock"
                }
            )
        
        # 检查是否已有升级在运行
        current_status = _read_upgrade_status()
        if current_status.get("status") == "running":
            return {
                "success": False,
                "message": "An upgrade task is already running",
                "data": current_status
            }
        
        # 验证文件扩展名
        if not file.filename.endswith('.run'):
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "success": False,
                    "message": "Only .run format upgrade package is supported"
                }
            )
        
        logger.info(f"开始接收升级包: {file.filename}")
        
        # 清空升级目录（避免旧文件权限问题）
        logger.info("清空升级目录...")
        for item in UPGRADE_DIR.iterdir():
            try:
                if item.is_file():
                    item.unlink()
                    logger.debug(f"删除文件: {item.name}")
                elif item.is_dir():
                    shutil.rmtree(item)
                    logger.debug(f"删除目录: {item.name}")
            except PermissionError as e:
                logger.warning(f"无法删除 {item.name}，将尝试继续: {e}")
            except Exception as e:
                logger.warning(f"删除 {item.name} 时出错: {e}")
        logger.info("升级目录已清空")
        
        # 重置中断标志
        global _upload_abort_flag
        _upload_abort_flag = False
        
        # 保存升级包到持久化目录（使用流式写入）
        upgrade_file_path = UPGRADE_DIR / file.filename
        
        # 限制文件大小（500MB）
        max_size = 500 * 1024 * 1024
        chunk_size = 1024 * 1024  # 1MB chunks
        total_size = 0
        
        try:
            with open(upgrade_file_path, 'wb') as f:
                while True:
                    # 检查中断标志
                    if _upload_abort_flag:
                        logger.warning("检测到上传中断请求")
                        raise Exception("上传被用户中断")
                    
                    # 分块读取
                    chunk = await file.read(chunk_size)
                    if not chunk:
                        break
                    
                    total_size += len(chunk)
                    
                    # 检查文件大小
                    if total_size > max_size:
                        raise Exception(f"文件大小超过限制（最大500MB），当前: {round(total_size / (1024 * 1024), 2)}MB")
                    
                    # 写入文件
                    f.write(chunk)
                    
                    # 每 10MB 记录一次进度
                    if total_size % (10 * 1024 * 1024) < chunk_size:
                        logger.info(f"上传进度: {round(total_size / (1024 * 1024), 2)}MB")
            
            if total_size == 0:
                raise Exception("上传的文件为空")
            
            logger.info(f"升级包接收完成: {file.filename}, 大小: {round(total_size / (1024 * 1024), 2)}MB")
            
        except Exception as e:
            # 清理不完整的文件
            if upgrade_file_path.exists():
                try:
                    upgrade_file_path.unlink()
                    logger.info(f"已清理不完整的上传文件: {upgrade_file_path}")
                except:
                    pass
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "success": False,
                    "message": f"File upload failed: {str(e)}"
                }
            )
        
        # 添加执行权限
        try:
            os.chmod(upgrade_file_path, 0o755)
        except PermissionError as e:
            # 如果无法修改权限（文件所有者是其他用户），记录警告但继续
            # 因为文件仍然可能是可执行的
            logger.warning(f"无法修改文件权限，但文件已保存: {e}")
        
        logger.info(f"升级包已保存: {upgrade_file_path}")
        
        # 初始化升级日志
        with open(UPGRADE_LOG_FILE, 'w') as log_f:
            log_f.write(f"=== VoltageEMS Upgrade Log ===\n")
            log_f.write(f"Started at: {datetime.now().isoformat()}\n")
            log_f.write(f"Package: {file.filename}\n")
            log_f.write(f"File size: {round(total_size / (1024 * 1024), 2)} MB\n")
            log_f.write("=" * 60 + "\n\n")
        
        # 创建宿主机执行脚本
        # 关键点：通过 docker exec 在宿主机上执行（而不是容器内）
        host_script = UPGRADE_DIR / "execute_on_host.sh"
        with open(host_script, 'w') as f:
            f.write("#!/bin/bash\n")
            f.write("# 此脚本在宿主机上执行，负责升级 VoltageEMS\n\n")
            f.write("set -euo pipefail\n\n")
            f.write(f"UPGRADE_FILE='{upgrade_file_path}'\n")
            f.write(f"LOG_FILE='{UPGRADE_LOG_FILE}'\n")
            f.write(f"STATUS_FILE='{UPGRADE_STATUS_FILE}'\n")
            f.write("INSTALL_DIR='/opt/MonarchEdge'\n\n")
            
            # 更新状态为运行中
            f.write("# Mark upgrade start\n")
            f.write('echo \'{"status": "running", "started_at": "\' >> "$STATUS_FILE"\n')
            f.write("date -Iseconds | tr -d '\\n' >> \"$STATUS_FILE\"\n")
            f.write('echo \'", "message": "Upgrading, please wait..."}\' >> "$STATUS_FILE"\n\n')
            
            # 记录日志
            f.write("echo 'Executing upgrade...' | tee -a \"$LOG_FILE\"\n")
            f.write("echo 'Upgrade file: '$UPGRADE_FILE | tee -a \"$LOG_FILE\"\n\n")
            
            # 执行升级包（自动模式）
            # 注意：Makeself 需要使用 -- 来传递参数给内嵌脚本
            f.write("# Execute upgrade package (install.sh --auto)\n")
            f.write("if \"$UPGRADE_FILE\" -- --auto >> \"$LOG_FILE\" 2>&1; then\n")
            f.write("    EXIT_CODE=0\n")
            f.write("    echo 'Upgrade completed successfully' | tee -a \"$LOG_FILE\"\n")
            f.write('    echo \'{"status": "completed", "finished_at": "\' > "$STATUS_FILE"\n')
            f.write("    date -Iseconds | tr -d '\\n' >> \"$STATUS_FILE\"\n")
            f.write('    echo \'", "exit_code": 0, "message": "Upgrade successful"}\' >> "$STATUS_FILE"\n')
            f.write("else\n")
            f.write("    EXIT_CODE=$?\n")
            f.write("    echo \"Upgrade failed with exit code: $EXIT_CODE\" | tee -a \"$LOG_FILE\"\n")
            f.write('    echo \'{"status": "failed", "finished_at": "\' > "$STATUS_FILE"\n')
            f.write("    date -Iseconds | tr -d '\\n' >> \"$STATUS_FILE\"\n")
            f.write('    echo \'", "exit_code": \'$EXIT_CODE\', "message": "Upgrade failed"}\' >> "$STATUS_FILE"\n')
            f.write("fi\n\n")
            
            # 清理升级包（可选）
            f.write("# Cleanup upgrade package\n")
            f.write("# rm -f \"$UPGRADE_FILE\"\n\n")
            
            f.write("exit $EXIT_CODE\n")
        
        # 添加执行权限
        try:
            os.chmod(host_script, 0o755)
        except PermissionError as e:
            logger.warning(f"无法修改脚本权限，但脚本已保存: {e}")
        
        # 使用 Docker Socket 在宿主机上执行脚本
        # 注意：这里不是在容器内执行，而是通过 Docker 在宿主机上执行
        logger.info("准备在宿主机上启动升级进程...")
        
        # 方法：直接在宿主机上执行脚本（通过 docker run 使用宿主机的 PID namespace）
        # 但更简单的方式是：让脚本自己执行，因为 /opt/MonarchEdge 是挂载的
        
        # 创建一个简单的触发器：写入一个标记文件，由外部监控器检测
        trigger_script = UPGRADE_DIR / "run_upgrade.sh"
        with open(trigger_script, 'w') as f:
            f.write("#!/bin/bash\n")
            f.write("# 在后台执行升级脚本\n")
            f.write(f"nohup {host_script} > /dev/null 2>&1 &\n")
            f.write("echo $! > " + str(UPGRADE_DIR / "upgrade.pid") + "\n")
        
        os.chmod(trigger_script, 0o755)
        
        # 通过 Docker Socket 创建一个临时容器在宿主机上执行
        # 关键：使用 nsenter 直接在宿主机命名空间执行（无需安装额外工具）
        # nsenter 在 alpine 中默认可用，可以进入宿主机的所有命名空间
        docker_cmd = [
            "docker", "run", "--rm", "-d",
            "--name", "voltageems-upgrader",
            "--pid", "host",  # 共享宿主机 PID 命名空间（nsenter 需要）
            "--privileged",   # 给予特权，nsenter 需要访问 /proc/1/ns/*
            "-v", "/opt/MonarchEdge:/opt/MonarchEdge",  # 挂载宿主机目录
            "alpine:latest",
            # nsenter 参数说明：
            # --target 1: 进入 PID 1 (init/systemd) 的命名空间（即宿主机）
            # --mount: 进入 mount 命名空间
            # --uts: 进入 UTS 命名空间（hostname）
            # --ipc: 进入 IPC 命名空间
            # --net: 进入 network 命名空间
            # --pid: 进入 PID 命名空间
            # --: 后面是要在宿主机上执行的命令
            "nsenter", "--target", "1", "--mount", "--uts", "--ipc", "--net", "--pid", "--",
            "bash", f"{host_script}"
        ]
        
        try:
            # 启动升级容器（增加超时时间，因为可能需要拉取镜像）
            # alpine 镜像通常很小（约5MB），但首次拉取可能需要时间
            result = subprocess.run(
                docker_cmd,
                capture_output=True,
                text=True,
                timeout=60  # 增加到60秒，给予足够时间拉取镜像
            )
            
            if result.returncode != 0:
                logger.error(f"启动升级容器失败: {result.stderr}")
                raise Exception(f"启动升级失败: {result.stderr}")
            
            container_id = result.stdout.strip()
            logger.info(f"升级容器已启动: {container_id[:12]}")
            
            # 写入初始状态
            _write_upgrade_status({
                "status": "running",
                "filename": file.filename,
                "size_mb": round(total_size / (1024 * 1024), 2),
                "started_at": datetime.now().isoformat(),
                "container_id": container_id[:12],
                "log_file": str(UPGRADE_LOG_FILE)
            })
            
            return {
                "success": True,
                "message": "Upgrade task started",
                "data": {
                    "filename": file.filename,
                    "size_mb": round(total_size / (1024 * 1024), 2),
                    "container_id": container_id[:12],
                    "log_file": str(UPGRADE_LOG_FILE),
                    "status_file": str(UPGRADE_STATUS_FILE),
                    "warning": "This container may be restarted during upgrade. Check status via status API",
                    "note": "Using smart update mode - only modified services will be restarted"
                }
            }
            
        except subprocess.TimeoutExpired:
            raise Exception("Upgrade container startup timeout")
        except Exception as e:
            logger.error(f"执行 Docker 命令失败: {e}")
            raise
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"上传并运行升级包异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={
                "success": False,
                "message": f"Upload and run upgrade package failed: {str(e)}"
            }
        )


@router.post("/upgrade/abort", response_model=Dict[str, Any])
async def abort_upgrade():
    """
    中断升级程序
    
    终止正在运行的升级进程或文件上传
    
    注意：
    - 如果升级容器正在运行，会停止该容器
    - 如果文件正在上传，会中断上传并清理不完整的文件
    """
    try:
        # 设置上传中断标志
        global _upload_abort_flag
        _upload_abort_flag = True
        logger.info("设置上传中断标志")
        
        # 读取当前状态
        status_data = _read_upgrade_status()
        
        if status_data.get("status") != "running":
            return {
                "success": True,
                "message": "Abort signal sent (may be uploading file)"
            }
        
        container_id = status_data.get("container_id")
        if not container_id:
            return {
                "success": True,
                "message": "Abort signal sent"
            }
        
        logger.info(f"尝试中断升级容器: {container_id}")
        
        # 停止升级容器
        try:
            subprocess.run(
                ["docker", "stop", container_id],
                capture_output=True,
                text=True,
                timeout=30
            )
            
            # 更新状态
            _write_upgrade_status({
                "status": "aborted",
                "filename": status_data.get("filename"),
                "aborted_at": datetime.now().isoformat()
            })
            
            # 记录到日志
            with open(UPGRADE_LOG_FILE, 'a') as log_f:
                log_f.write(f"\n\n!!! Upgrade aborted by user ({datetime.now().isoformat()}) !!!\n")
            
            return {
                "success": True,
                "message": "Upgrade aborted",
                "data": {
                    "container_id": container_id,
                    "log_file": str(UPGRADE_LOG_FILE)
                }
            }
            
        except subprocess.TimeoutExpired:
            raise Exception("Stop upgrade container timeout")
        except Exception as e:
            raise Exception(f"Failed to stop upgrade container: {str(e)}")
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"中断升级异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={
                "success": False,
                "message": f"Failed to abort upgrade: {str(e)}"
            }
        )


@router.get("/upgrade/status", response_model=Dict[str, Any])
async def get_upgrade_status():
    """
    获取升级状态
    
    返回当前升级任务的状态和完整日志
    
    状态说明：
    - running: 升级进行中
    - finished: 升级已结束（包括成功、失败、中断等所有结束状态）
    """
    try:
        # 确保升级目录存在
        _ensure_upgrade_dir()
        
        # 读取状态文件
        status_data = _read_upgrade_status()
        
        # 读取完整日志文件
        log_content = ""
        if UPGRADE_LOG_FILE.exists():
            try:
                with open(UPGRADE_LOG_FILE, 'r', encoding='utf-8') as f:
                    log_content = f.read()
            except Exception as e:
                logger.warning(f"读取日志文件失败: {e}")
                log_content = f"Failed to read log file: {str(e)}"
        else:
            log_content = "Log file not found"
        
        # 获取原始状态
        original_status = status_data.get("status", "idle")
        
        # 如果状态显示正在运行，检查容器是否真的在运行
        if original_status == "running":
            container_id = status_data.get("container_id")
            if container_id:
                try:
                    result = subprocess.run(
                        ["docker", "inspect", "-f", "{{.State.Running}}", container_id],
                        capture_output=True,
                        text=True,
                        timeout=5
                    )
                    
                    if result.returncode != 0 or result.stdout.strip().lower() != "true":
                        # 容器已停止，但状态文件未更新
                        # 尝试获取退出码
                        exit_code_result = subprocess.run(
                            ["docker", "inspect", "-f", "{{.State.ExitCode}}", container_id],
                            capture_output=True,
                            text=True,
                            timeout=5
                        )
                        
                        exit_code = 0
                        if exit_code_result.returncode == 0:
                            try:
                                exit_code = int(exit_code_result.stdout.strip())
                            except:
                                pass
                        
                        # 更新状态
                        _write_upgrade_status({
                            "status": "completed" if exit_code == 0 else "failed",
                            "filename": status_data.get("filename"),
                            "finished_at": datetime.now().isoformat(),
                            "exit_code": exit_code
                        })
                        
                        status_data = _read_upgrade_status()
                        original_status = status_data.get("status", "idle")
                        
                except Exception as e:
                    logger.warning(f"检查容器状态失败: {e}")
        
        # 统一状态：running 或 finished
        if original_status == "running":
            unified_status = "running"
        else:
            # completed, failed, aborted, idle 都归为 finished
            unified_status = "finished"
        
        return {
            "success": True,
            "data": {
                "status": unified_status,
                "log_file": str(UPGRADE_LOG_FILE),
                "log_preview": log_content,  # 完整日志内容
                **{k: v for k, v in status_data.items() if k != "status"}  # 其他状态信息（排除原始 status）
            }
        }
        
    except Exception as e:
        logger.error(f"获取升级状态异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={
                "success": False,
                "message": f"Failed to get upgrade status: {str(e)}"
            }
        )

