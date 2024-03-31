package mqtt_test

import (
	"fmt"
	"io"
	"log"
	"os"
	"os/exec"
	"testing"
	"time"

	"github.com/stretchr/testify/suite"
)

type MySuite struct {
	suite.Suite
	cmd *exec.Cmd
}

// build binary directory
const mqtt_server_binary = "./target/debug/mqtt-server"

// build application
func (suite *MySuite) SetupSuite() {
	suite.T().Log("Build start")
	cmd := exec.Command("cargo", "build")
	cmd.Dir = ".."
	stdErrorPipe, err := cmd.StderrPipe()
	if err != nil {
		suite.T().Fatal(err)
	}
	if err := cmd.Start(); err != nil {
		log.Fatal(err)
	}

	slurp, _ := io.ReadAll(stdErrorPipe)
	fmt.Printf("stderr: %s\n", slurp)

	if err := cmd.Wait(); err != nil {
		log.Fatal(err)
	}
}
func (suite *MySuite) TearDownSuite() {
	suite.T().Log("TearDownSuite")
}
func (suite *MySuite) SetupTest() {
	suite.T().Log(": SetupTest")
}
func (suite *MySuite) TearDownTest() {
	suite.T().Log(": TeardownTest")
}

// run test mqtt-server with args
func (suite *MySuite) BeforeTest(suiteName, testName string) {
	suite.T().Log(": BeforeTest")
	suite.cmd = exec.Command(mqtt_server_binary)
	suite.cmd.Stdout = os.Stdout
	suite.cmd.Dir = ".."
	//suite.cmd.WaitDelay = 1 * time.Second

	if err := suite.cmd.Start(); err != nil {
		log.Fatal(err)
	}

	// suiteName, testNameによってargsを変更する

}

// stop test mqtt-mqttserver
func (suite *MySuite) AfterTest(suiteName, testName string) {
	suite.T().Log(": AfterTest")
	err := suite.cmd.Process.Signal(os.Interrupt)
	if err != nil {
		_, _ = fmt.Fprintf(os.Stderr, "Signal: "+err.Error())
		return
	}
}
func (suite *MySuite) TestExec1() {
	suite.T().Log(": TestExec1")

	suite.T().Log("Hello Sleeping")
	time.Sleep(time.Second * 10)
	suite.T().Log("End Sleeping")
	suite.Equal(true, true)

}
func (suite *MySuite) TestExec2() {
	suite.T().Log(": TestExec2")
	suite.T().Log("Hello Sleeping")
	time.Sleep(time.Second * 10)
	suite.T().Log("End Sleeping")
	suite.Equal(true, true)
}

// テスト実行
func TestMySuite(t *testing.T) {
	suite.Run(t, new(MySuite))
}
