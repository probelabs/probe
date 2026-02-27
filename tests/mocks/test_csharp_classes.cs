using System;
using System.Collections.Generic;

namespace Example
{
    public class UserController
    {
        private readonly IUserService _service;

        public UserController(IUserService service)
        {
            _service = service;
        }

        public List<User> GetAll()
        {
            return _service.GetAllUsers();
        }

        public User FindById(int id)
        {
            return _service.FindUser(id);
        }

        public void Delete(int id)
        {
            _service.DeleteUser(id);
        }
    }

    public class ProductController
    {
        public List<Product> GetAll()
        {
            return new List<Product>();
        }

        public void Update(Product product)
        {
            // update product
        }
    }
}
