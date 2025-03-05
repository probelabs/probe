package main

import (
	"net/http"

	"github.com/gin-gonic/gin"
)

func HandleNotFound(c *gin.Context) {
	c.JSON(http.StatusNotFound, ErrorResponse{
		Errors: []struct {
			Title  string `json:"title"`
			Detail string `json:"detail"`
		}{{Title: "Not Found", Detail: "Model price not found"}},
	})
}

type ErrorResponse struct {
	Errors interface{} `json:"errors"`
}

func main() {
	r := gin.Default()
	r.NoRoute(HandleNotFound)
	r.Run()
}
