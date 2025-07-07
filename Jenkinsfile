pipeline {
    agent any
    
    environment {
        // Docker镜像版本
        VERSION = "${env.BUILD_NUMBER}-${env.GIT_COMMIT.take(7)}"
        // Docker Registry地址（本地或边缘服务器）
        REGISTRY = "localhost:5000"
        // 项目名称
        PROJECT = "voltageems"
    }
    
    stages {
        stage('准备') {
            steps {
                echo "开始构建 VoltageEMS v${VERSION}"
                sh 'docker version'
                sh 'docker-compose version'
            }
        }
        
        stage('构建') {
            steps {
                echo "构建所有Docker镜像..."
                sh '''
                    chmod +x scripts/build-all.sh
                    ./scripts/build-all.sh ${VERSION} ${REGISTRY}
                '''
            }
        }
        
        stage('测试') {
            parallel {
                stage('单元测试') {
                    steps {
                        echo "运行单元测试..."
                        sh '''
                            # 在容器中运行Rust测试
                            docker run --rm ${REGISTRY}/${PROJECT}/comsrv:${VERSION} cargo test
                        '''
                    }
                }
                stage('集成测试') {
                    steps {
                        echo "运行集成测试..."
                        sh '''
                            # 启动测试环境
                            docker-compose -f docker-compose.test.yml up -d
                            sleep 10
                            
                            # 运行测试
                            ./scripts/run-integration-tests.sh
                            
                            # 清理
                            docker-compose -f docker-compose.test.yml down
                        '''
                    }
                }
            }
        }
        
        stage('推送镜像') {
            steps {
                echo "推送镜像到Registry..."
                sh '''
                    # 推送所有镜像到本地Registry
                    for service in comsrv modsrv hissrv netsrv alarmsrv apigateway frontend; do
                        docker push ${REGISTRY}/${PROJECT}/${service}:${VERSION}
                        docker push ${REGISTRY}/${PROJECT}/${service}:latest
                    done
                '''
            }
        }
        
        stage('部署') {
            when {
                branch 'main'
            }
            steps {
                echo "部署到生产环境..."
                sh '''
                    chmod +x scripts/deploy.sh
                    ./scripts/deploy.sh production ${VERSION}
                '''
            }
        }
    }
    
    post {
        success {
            echo "构建成功！版本: ${VERSION}"
        }
        failure {
            echo "构建失败！"
            sh 'docker-compose -f docker-compose.test.yml down || true'
        }
        always {
            // 清理工作空间
            sh 'docker system prune -f'
        }
    }
}