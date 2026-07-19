package cmd

import (
	pb "github.com/shadowlynx/prox-cli/pkg/grpc/proto"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

func getClient() (pb.OrchestratorClient, *grpc.ClientConn, error) {
	conn, err := grpc.NewClient(
		"localhost:50052",
		grpc.WithTransportCredentials(insecure.NewCredentials()),
	)
	if err != nil {
		return nil, nil, err
	}
	return pb.NewOrchestratorClient(conn), conn, nil
}
