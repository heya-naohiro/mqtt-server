package mqtt_test

import (
	"crypto/tls"
	"fmt"
	"io"
	"log"
	"os"
	"os/exec"
	"testing"
	"time"

	mqtt "github.com/eclipse/paho.mqtt.golang"

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
		suite.T().Error(err)
	}
	if err := cmd.Start(); err != nil {
		suite.T().Error(err)

	}

	slurp, _ := io.ReadAll(stdErrorPipe)
	fmt.Printf("stderr: %s\n", slurp)

	if err := cmd.Wait(); err != nil {
		suite.T().Error(err)
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
		suite.T().Error(err)
	}
	time.Sleep(time.Second * 1)
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

func subscribeWaitFirstPacket(c mqtt.Client, topic string, qos byte) (chan mqtt.Message, error) {
	channel := make(chan mqtt.Message)
	var f mqtt.MessageHandler = func(client mqtt.Client, msg mqtt.Message) {
		log.Printf("Sub data: %v\n", msg)
		channel <- msg
	}

	if token := c.Subscribe(topic, qos, f); token.Wait() && token.Error() != nil {
		log.Printf("Mqtt subscribe error %s", token.Error())
		return nil, token.Error()
	} else {
		log.Println("Subscribe Success")
	}
	return channel, nil
}

func (suite *MySuite) TestConnectPublishSubscribe() {
	opts := mqtt.NewClientOptions()
	opts.ClientID = "client_1"
	opts.AddBroker("tls://localhost:8883")
	opts.WillEnabled = false
	tlsConfig := &tls.Config{
		InsecureSkipVerify: true,
	}
	opts.SetTLSConfig(tlsConfig)
	opts.SetCleanSession(true)
	c := mqtt.NewClient(opts)
	if token := c.Connect(); token.Wait() && token.Error() != nil {
		suite.T().Errorf("Mqtt error: %s", token.Error())
	}

	msgc, err := subscribeWaitFirstPacket(c, "topic/any", 0)
	if err != nil {
		suite.T().Error(err)
	}
	opts.ClientID = "client_2"
	opts.AddBroker("tls://localhost:8883")
	opts.WillEnabled = false
	opts.SetTLSConfig(tlsConfig)
	opts.SetCleanSession(true)
	c2 := mqtt.NewClient(opts)
	if token := c2.Connect(); token.Wait() && token.Error() != nil {
		suite.T().Errorf("Mqtt error: %s", token.Error())
	}
	expect := "test payload2"

	suite.T().Log("Publish Start")
	if token := c2.Publish("topic/any", 0, false, []byte(expect)); token.Wait() && token.Error() != nil {
		suite.T().Error(token.Error())
	}
	suite.T().Log("Publish End")
	var msg mqtt.Message
	select {
	case resMsg := <-msgc:
		if string(resMsg.Payload()) != expect {
			suite.T().Errorf("Expect: %s, but resMsg: %s", expect, string(resMsg.Payload()))
		}
	case <-time.After(100 * time.Second):
		suite.T().Error("Error timeout subscription")
	}
	suite.T().Logf("Success %s", msg)

}

// テスト実行
func TestMySuite(t *testing.T) {
	suite.Run(t, new(MySuite))
}
