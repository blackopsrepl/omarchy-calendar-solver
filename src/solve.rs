use crate::error::AppError;
use crate::model::Proposal;
use crate::protocol::Request;

pub fn solve(request: &Request) -> Result<Proposal, AppError> {
    crate::scheduling::solve(request)
}
