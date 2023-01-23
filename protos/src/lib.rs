tonic::include_proto!("orderbook");

impl TryFrom<(&str, &[String; 2])> for Level {
    type Error = ();

    fn try_from((ex, price_amount): (&str, &[String; 2])) -> Result<Self, Self::Error> {
        let price = price_amount[0].parse().map_err(|_| ())?;
        let amount = price_amount[1].parse().map_err(|_| ())?;
        Ok(Self {
            exchange: ex.into(),
            price,
            amount,
        })
    }
}
