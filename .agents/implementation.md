# Repository Guidance

Before planning or changing behavior, discover and read applicable `design/*.md`. Use `base-design-review` to assess plans and changes, and `base-design-author` when user requirements are introduced, changed, or removed. Review design compliance before opening or updating a pull request.

Install shared guidance with `cargo bake agent:context:install`. Discover it with `cargo bake agent:context:list` and `cargo bake agent:context:show`. Keep library runtime dependencies separate from the private `bake/` package.
