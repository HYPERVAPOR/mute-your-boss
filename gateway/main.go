package main

import (
	"context"
	"log"
	"net/http"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

	pb "github.com/HYPERVAPOR/mute-your-boss/gateway/internal/pb"
)

const (
	coreAddr = "127.0.0.1:50051"
	httpAddr = "127.0.0.1:8080"
)

func main() {
	conn, err := grpc.NewClient(coreAddr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("failed to connect to core: %v", err)
	}
	defer conn.Close()

	client := pb.NewMuteYourBossClient(conn)

	mux := http.NewServeMux()
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer cancel()

		_, err := client.GetStatus(ctx, &pb.SessionRef{SessionId: ""})
		if err != nil {
			w.WriteHeader(http.StatusServiceUnavailable)
			return
		}
		w.Write([]byte("ok"))
	})

	log.Printf("myb-gateway listening on %s", httpAddr)
	if err := http.ListenAndServe(httpAddr, mux); err != nil {
		log.Fatalf("gateway failed: %v", err)
	}
}
