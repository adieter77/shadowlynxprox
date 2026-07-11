package grpc

import (
	"context"
	"fmt"
	"io"
	"time"

	pb "github.com/shadowlynx/prox-cli/pkg/grpc/orchestrator"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

// Client wraps the gRPC connection to the orchestrator
type Client struct {
	conn       *grpc.ClientConn
	grpcClient pb.OrchestratorClient
	addr       string
}

// ClientConfig holds connection settings
type ClientConfig struct {
	Address        string
	ConnectTimeout time.Duration
}

// DefaultClientConfig returns sensible defaults
func DefaultClientConfig() ClientConfig {
	return ClientConfig{
		Address:        "localhost:50052",
		ConnectTimeout: 10 * time.Second,
	}
}

// NewClient creates a new gRPC client connected to the orchestrator
func NewClient(config ClientConfig) (*Client, error) {
	if config.Address == "" {
		config.Address = "localhost:50052"
	}
	if config.ConnectTimeout == 0 {
		config.ConnectTimeout = 10 * time.Second
	}

	ctx, cancel := context.WithTimeout(context.Background(), config.ConnectTimeout)
	defer cancel()

	// Dial the orchestrator
	// Using insecure credentials for local development
	// In production, this should use TLS
	conn, err := grpc.DialContext(
		ctx,
		config.Address,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithBlock(), // Wait for connection before returning
	)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to orchestrator at %s: %w", config.Address, err)
	}

	return &Client{
		conn:       conn,
		grpcClient: pb.NewOrchestratorClient(conn),
		addr:       config.Address,
	}, nil
}

// Close shuts down the gRPC connection
func (c *Client) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

// Address returns the orchestrator address this client is connected to
func (c *Client) Address() string {
	return c.addr
}

// HealthCheck checks if the orchestrator is healthy
func (c *Client) HealthCheck(ctx context.Context) (*pb.HealthCheckResponse, error) {
	resp, err := c.grpcClient.HealthCheck(ctx, &pb.HealthCheckRequest{})
	if err != nil {
		return nil, fmt.Errorf("health check failed: %w", err)
	}
	return resp, nil
}

// GetInfo retrieves orchestrator version and capabilities
func (c *Client) GetInfo(ctx context.Context) (*pb.GetInfoResponse, error) {
	resp, err := c.grpcClient.GetInfo(ctx, &pb.GetInfoRequest{})
	if err != nil {
		return nil, fmt.Errorf("get info failed: %w", err)
	}
	return resp, nil
}

// Execute sends a one-shot command to the orchestrator
func (c *Client) Execute(ctx context.Context, req *pb.ExecuteRequest) (*pb.ExecuteResponse, error) {
	resp, err := c.grpcClient.Execute(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("execute failed: %w", err)
	}
	return resp, nil
}

// ChatHandler is a callback that receives streaming chat responses
type ChatHandler func(chunk *pb.ChatResponse) error

// Chat starts a streaming chat session with the orchestrator
func (c *Client) Chat(ctx context.Context, req *pb.ChatRequest, handler ChatHandler) error {
	stream, err := c.grpcClient.Chat(ctx, req)
	if err != nil {
		return fmt.Errorf("chat stream failed: %w", err)
	}

	for {
		resp, err := stream.Recv()
		if err == io.EOF {
			// Stream ended normally
			return nil
		}
		if err != nil {
			return fmt.Errorf("error receiving chat response: %w", err)
		}

		// Call the handler for each chunk
		if err := handler(resp); err != nil {
			return fmt.Errorf("handler error: %w", err)
		}

		// If this is the final message, we're done
		if resp.IsFinal {
			return nil
		}
	}
}
